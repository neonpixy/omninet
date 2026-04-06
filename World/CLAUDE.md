# World — Digital & Physical

Everything outside the castle walls. World is bifurcated into the digital network and the physical bridge.

## World/Digital/ — The Omninet

Network infrastructure that connects sovereign nodes.

| Component | What It Is |
|-----------|-----------|
| **Omnibus** | Node runtime. Every device running Omnibus is a sovereign node. Boots relay server, discovery, identity, pool in one call. Embedded on mobile, managed by Omny on desktop. |
| **Tower** | Always-on network nodes. Pharos mode (gospel-only directory) and Harbor mode (community content storage). Lighthouse announcements (kind 7032) advertise capabilities. Event filtering via Globe's EventFilter. Gospel peering loop syncs identity/trust data between towers. Full-text search via MagicalIndex. `omny-tower` CLI binary. |
| **MagicalIndex** | Demand-driven search engine inside Tower nodes. FTS5 keyword search (BM25 ranking, Porter stemmer, snippet extraction), compound queries (tag filters, kind/author/time filters, custom sorting), aggregation (count, sum, min/max, avg, group-by), faceted search, Zeitgeist query signals. Index grows from human curiosity, not crawling. |

Push notifications live in Divinity (platform-specific: APNs, FCM, Web Push).

**Every desktop is infrastructure by default.** When you install Omny, network features are ON — your machine participates as a lightweight Pharos node, caching gospel and serving search results. Capped at 20GB.

## World/Physical/ — The Land

Geographic presence, local communities, proximity verification, Cash distribution. Eight modules fully built: Place, Region, Rendezvous, Presence, Lantern, Handoff, OmniTag, Caravan. Plus a builder module with Globe event kind constants (23000-24000) and tag/content helpers.

## Dependencies

Digital: Crown (identity), Globe (relay, pool, discovery, naming)
Physical: X (GeoCoordinate, point_in_polygon), Crown (identity), Bulwark (ProximityProof)
