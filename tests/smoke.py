"""Check a real Rust server over HTTP using a temporary SQLite database.

Run after `cargo build`: python3 tests/smoke.py
Set CHECK_LIVE_WEATHER=1 to also verify the external forecast provider.
"""
import json
import os
from pathlib import Path
import subprocess
import sqlite3
import tempfile
import time
import urllib.error
import urllib.request
from datetime import date, timedelta

ROOT = Path(__file__).resolve().parents[1]
BASE = "http://127.0.0.1:38087"


def request(method, path, body=None, credential=None):
    headers = {"Content-Type": "application/json"}
    if credential:
        headers["Authorization"] = "Bearer " + credential
    payload = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(BASE + path, payload, headers, method=method)
    try:
        response = urllib.request.urlopen(req, timeout=90)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        data = response.read()
        return response.status, json.loads(data), response.headers


def saturday_after(day):
    day += timedelta(days=1)
    while day.weekday() != 5:
        day += timedelta(days=1)
    return day.isoformat()


with tempfile.TemporaryDirectory(prefix="weekend-smoke-") as directory:
    environment = dict(os.environ, HOST="127.0.0.1", PORT="38087", DATABASE_PATH=directory + "/app.sqlite", STATIC_DIR=str(ROOT / "static"), RUST_LOG="error")
    process = subprocess.Popen([str(ROOT / "target/debug/weekend-club")], cwd=ROOT, env=environment, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    try:
        ready = False
        for _ in range(100):
            if process.poll() is not None:
                raise RuntimeError(process.stdout.read().decode())
            try:
                if request("GET", "/api/health")[0] == 200:
                    ready = True
                    break
            except urllib.error.URLError:
                time.sleep(0.1)
        assert ready, "Server did not start"
        status, catalog, headers = request("GET", "/api/mountains")
        assert status == 200 and len(catalog["mountains"]) == 41
        assert "frame-ancestors 'none'" in headers["Content-Security-Policy"]
        today = date.fromisoformat(catalog["today"])
        weekend = saturday_after(today)
        second = (date.fromisoformat(weekend) + timedelta(days=7)).isoformat()
        trip_options = ["mammoth@" + weekend, "palisades@" + second]
        selection = {"username": "test_owner", "title": "Test planner", "mountains": ["mammoth", "palisades"], "weekends": [weekend, second], "trips": trip_options}
        status, created, _ = request("POST", "/api/plans", selection)
        assert status == 200
        planner = "/api/plans/" + created["plan"]["id"]
        owner_credential = created["owner_token"]
        status, public, _ = request("GET", planner)
        assert status == 200 and "owner_token" not in public
        assert owner_credential not in json.dumps(public)
        assert request("PATCH", planner, selection)[0] == 403
        assert request("PUT", planner + "/availability", {"username": "test_friend", "trips": ["alta@" + weekend]})[0] == 400
        assert request("PUT", planner + "/availability", {"username": "test_friend", "trips": ["mammoth@" + second]})[0] == 400
        choices = {"username": "test_friend", "trips": trip_options}
        status, joined, _ = request("PUT", planner + "/availability", choices)
        assert status == 200 and len(joined["plan"]["members"]) == 1
        assert request("PUT", planner + "/availability", choices)[0] == 409
        assert request("PUT", planner + "/availability", {"username": "test_friend", "trips": []}, joined["member_token"])[0] == 200
        assert request("PUT", planner + "/availability", choices, joined["member_token"])[0] == 200
        selection["mountains"] = ["mammoth"]
        selection["weekends"] = [weekend]
        selection["trips"] = ["mammoth@" + weekend]
        status, edited, _ = request("PATCH", planner, selection, owner_credential)
        assert status == 200 and edited["members"][0]["trips"] == ["mammoth@" + weekend]
        far = saturday_after(today + timedelta(days=60))
        if os.environ.get("CHECK_LIVE_WEATHER") != "1":
            # Deterministic fixture for CI; real archival data is checked in live mode.
            day = date(today.year-5,1,1)
            last = date(today.year-1,12,31)
            days = []
            while day <= last:
                days.append(day.isoformat())
                day += timedelta(days=1)
            count = len(days)
            fixture = {"daily": {"time": days, "temperature_2m_max": [40.0]*count, "temperature_2m_min": [20.0]*count, "snowfall_sum": [2.54]*count, "wind_speed_10m_max": [10.0]*count}, "daily_units": {"snowfall_sum": "cm"}}
            with sqlite3.connect(directory + "/app.sqlite") as db:
                db.execute("INSERT INTO climate_cache VALUES (?,?,?)", ("mammoth", int(time.time()), json.dumps(fixture)))
        status, weather, _ = request("POST", "/api/weather", {"mountains": ["mammoth"], "weekends": [far]})
        estimate = weather["forecasts"][0]
        assert status == 200 and estimate["status"] == "estimate", estimate["status"]
        assert len(estimate["days"]) == 2 and estimate["estimate_basis"]["end_year"] == today.year-1
        assert all(day["code"] is None for day in estimate["days"])
        if os.environ.get("CHECK_LIVE_WEATHER") == "1":
            print("Live seasonal estimate: five years of archival data, two trip days, explicit estimate label.")
        for path in ["/", created["share_path"], "/app.js", "/style.css", "/assets/alpine.jpg"]:
            with urllib.request.urlopen(BASE + path) as response:
                assert response.status == 200
                assert len(response.read()) > 100
        if os.environ.get("CHECK_LIVE_WEATHER") == "1":
            status, weather, _ = request("POST", "/api/weather", {"mountains": ["mammoth"], "weekends": [weekend]})
            forecast = weather["forecasts"][0]
            assert status == 200 and forecast["status"] == "available", forecast["status"]
            assert len(forecast["days"]) == 2 and forecast["updated_at"]
            assert all(day["snowfall_inches"] >= 0 for day in forecast["days"])
            print("Live forecast: two complete days returned, units normalized, cache timestamp present.")
        print("HTTP checks passed: planner creation, owner authorization, restricted friend choices, participant updates, owner edits, seasonal estimates, frontend assets.")
    finally:
        process.terminate()
        try:
            process.wait(timeout=15)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
