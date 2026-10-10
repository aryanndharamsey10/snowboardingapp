import {test} from 'node:test';
import assert from 'node:assert/strict';
import {routePath,routeUrl,apiUrl} from '../static/hosting.mjs';

const base = new URL('https://aryanndharamsey10.github.io/snowboardingapp/');
const id = 'a'.repeat(32);
test('invites retain the Pages project path and survive direct opening', () => {
  const invite = routeUrl(base, `/p/${id}`);
  assert.equal(invite, `${base.href}#/p/${id}`);
  assert.equal(routePath(new URL(invite)), `/p/${id}`);
  assert.equal(routePath(new URL(base)), '/');
  assert.equal(routePath(new URL(`http://localhost:3000/p/${id}`)), `/p/${id}`);
});
test('navigation keeps frontend and backend addresses separate', () => {
  assert.equal(routeUrl(base, '/new'), `${base.href}#/new`);
  assert.equal(apiUrl(base, 'https://example.trycloudflare.com', '/api/plans'), 'https://example.trycloudflare.com/api/plans');
  assert.equal(apiUrl(new URL('http://localhost:3000/'), '', '/api/mountains'), 'http://localhost:3000/api/mountains');
});
