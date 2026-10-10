#!/usr/bin/env python3
"""Configure or inspect Pages using the existing Git credential helper.

Credentials remain in memory and are never printed or written to a file.
"""
import json
import subprocess
import sys
import urllib.error
import urllib.request

REPO = 'aryanndharamsey10/snowboardingapp'
credential = subprocess.run(
    ['git', 'credential', 'fill'], input='protocol=https\nhost=github.com\n\n',
    text=True, capture_output=True, check=True,
)
fields = dict(line.split('=', 1) for line in credential.stdout.splitlines() if '=' in line)
secret = fields.get('password')
if not secret:
    sys.exit('GitHub authentication is unavailable. Enable Pages in repository Settings > Pages.')


def api(method, path, body=None):
    request = urllib.request.Request('https://api.github.com/repos/' + REPO + path,
        data=json.dumps(body).encode() if body is not None else None, method=method,
        headers={'Authorization': 'Bearer ' + secret, 'Accept': 'application/vnd.github+json',
                 'Content-Type': 'application/json', 'X-GitHub-Api-Version': '2022-11-28'})
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            data = response.read()
            return json.loads(data) if data else {}
    except urllib.error.HTTPError as error:
        if error.code == 404 and method == 'GET' and path == '/pages':
            return None
        details = json.loads(error.read()).get('message', 'Request failed')
        sys.exit(f'GitHub API {error.code}: {details}')


if len(sys.argv) > 1 and sys.argv[1] == 'enable':
    current = api('GET', '/pages')
    if current is None:
        current = api('POST', '/pages', {'build_type': 'workflow'})
    elif current.get('build_type') != 'workflow':
        current = api('PUT', '/pages', {'build_type': 'workflow'})
    print('Pages enabled:', current.get('html_url'), current.get('build_type'))
else:
    current = api('GET', '/pages')
    print('Pages:', json.dumps({k: current.get(k) for k in ['html_url', 'status', 'build_type']}) if current else 'not enabled')
    runs = api('GET', '/actions/workflows/pages.yml/runs?per_page=3')
    for run in runs.get('workflow_runs', []):
        print('Deployment:', run['status'], run['conclusion'], run['html_url'])
