// Capacity-axis load script for the containerised sandbox.
//
// Drives the endpoints a real page hits and fails the run when latency/errors
// breach the thresholds — so a too-tight --cpus/--memory config shows up as a
// FAILED run, not a worse average. Keep the hit list to the scopes your seed()
// actually populates: a failing run should mean resource starvation, never
// missing data.
//
// Usage (see references/capacity-sizing.md for the sweep method):
//   BASE_URL=http://127.0.0.1:8070 VUS=50 DURATION=1m \
//     k6 run --summary-export cpu-1.json templates/stress.k6.js
//
// The load is the CONTROL variable: hold VUS/DURATION fixed across a sweep and
// vary only one resource axis (--cpus or --memory) between runs.

import http from 'k6/http'
import { check, sleep } from 'k6'

const BASE_URL = __ENV.BASE_URL || 'http://127.0.0.1:8070'
const VUS = Number(__ENV.VUS || 50)
const DURATION = __ENV.DURATION || '1m'

export const options = {
  scenarios: {
    steady: { executor: 'constant-vus', vus: VUS, duration: DURATION },
  },
  // Thresholds are the SLO. A run that breaches any of these exits non-zero,
  // which is what turns "the curve got worse" into a hard signal.
  thresholds: {
    http_req_duration: ['p(95)<800', 'p(99)<2000'],
    http_req_failed: ['rate<0.01'],
  },
}

// Replace with the endpoints a real page actually hits, restricted to the
// scopes your seed() populates.
const ENDPOINTS = ['/health', '/api/example']

export default function () {
  for (const path of ENDPOINTS) {
    const res = http.get(`${BASE_URL}${path}`)
    check(res, { 'status is 2xx': (r) => r.status >= 200 && r.status < 300 })
  }
  sleep(1)
}
