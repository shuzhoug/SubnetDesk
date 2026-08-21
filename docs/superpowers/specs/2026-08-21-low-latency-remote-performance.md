# SubnetDesk Low-Latency Remote Performance Requirements

Date: 2026-08-21

## Goal

Enterprise management features must not materially degrade SubnetDesk remote desktop responsiveness. The cloud control plane is used for identity, authorization, device presence, policy, and audit only. Screen video, audio, input, clipboard, and file-transfer traffic remain on the existing direct SubnetDesk data path whenever network routing permits.

## Low-Latency Mode

Add a user-selectable `Low Latency` remote-session profile while retaining normal quality-oriented profiles.

Low Latency mode prioritizes input responsiveness and frame freshness over maximum visual quality.

### Required behavior

- Prefer hardware video encoding when a supported encoder is available.
- Prefer hardware decoding when supported by the controller device.
- Dynamically adjust bitrate according to measured available bandwidth and packet loss.
- Dynamically adjust frame rate according to network and encoder load.
- Avoid building a large frame queue; stale frames should be dropped rather than displayed late.
- Reduce quality/resolution before allowing latency to grow excessively on constrained links.
- Preserve mouse and keyboard event priority over non-critical background traffic.
- Keep clipboard and file-transfer workloads from starving interactive remote-control traffic.
- Do not send remote desktop media through Laravel or Device Gateway.

## Initial Session Profiles

### Balanced

Default profile for normal office use.

- adaptive quality;
- adaptive bitrate;
- adaptive frame rate;
- hardware codec preferred;
- moderate visual-quality bias.

### Low Latency

Optimized for interactive operation.

- hardware codec strongly preferred;
- aggressive adaptive bitrate;
- aggressive frame dropping when frames become stale;
- responsiveness prioritized over image sharpness;
- input traffic prioritized;
- automatic resolution/quality reduction on weak links.

### High Quality

Optimized for viewing static or detailed content when network conditions are strong.

- higher image-quality target;
- less aggressive quality reduction;
- latency may be moderately higher than Low Latency mode.

## Measurements

The client should expose internal session metrics so performance can be tested and tuned:

- network RTT;
- effective video bitrate;
- current FPS;
- encode time;
- decode time;
- frame queue depth or estimated presentation delay;
- packet loss when available;
- active codec;
- whether hardware encoding/decoding is active.

These metrics may initially be visible only in a diagnostics/developer panel.

## Performance Rules

1. Enterprise authorization happens before session establishment and must not sit in the continuous media path.
2. Gateway heartbeats and policy messages must be independent of the remote desktop media transport.
3. MySQL, Redis, Laravel, or Gateway load must not directly throttle an already established peer-to-peer remote session.
4. Session audit writes are asynchronous from the user's interactive media path whenever possible.
5. No screen frame, audio frame, keystroke content, clipboard payload, or transferred file payload is uploaded to the enterprise control plane for ordinary audit purposes.

## Acceptance Tests

Phase-one remote performance validation must include:

- same-LAN 1080p office desktop control;
- same-LAN 2K desktop control on capable hardware;
- artificial 50 ms, 100 ms, and 200 ms RTT conditions;
- constrained-bandwidth tests;
- packet-loss tests;
- hardware-encoder enabled and software fallback tests;
- simultaneous enterprise Gateway heartbeat/control traffic while a remote session is active.

The acceptance criterion is that enabling enterprise management does not introduce a continuous control-plane dependency in the media path, and Low Latency mode reacts to degraded network conditions by reducing visual load before allowing large stale-frame queues to accumulate.
