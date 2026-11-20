# Teamobi2026 — Full Safe Rust MMORPG Server (`nro-server`)

Complete `#![forbid(unsafe_code)]` Rust `1.100.0` (`edition = "2024"`, `resolver = "3"`) port of the legacy 543-class Java/Ant server (`SRC/src/nro`, `94,028` non-empty LOC), `DataGame` binary asset streamer (`SRC/data`), and 41-table database (`database team2026.sql` / `SRC/sql/nro1.sql`), powered by embedded pure-Rust [`fjall`](https://crates.io/crates/fjall) LSM-tree storage.

## Workspace Architecture (9 Crates)

| Crate | Legacy Java Coverage | Key Architectural Highlights |
| --- | --- | --- |
| `crates/nro-core` | `interfaces/*`, `utils/Util`, `Location`, `Item` | Strongly typed IDs (`AccountId`, `PlayerId`, `MapId`, `ZoneId`, `TemplateId`, `BossTypeId`), `SlotMap` keys (`EntityKey`, `SessionKey`), `SmallVec<[ItemOption; 4]>`, `ParityMode`, crash-safe `ChaCha8Rng` |
| `crates/nro-consts` | `consts/*` (21 files) + `BossID.java` | 8,576 extracted constants across 15 zero-dependency modules |
| `crates/nro-protocol` | `network/*` (12), `Controller`, `MenuController`, `Command` | Out-of-place rolling XOR (`-27`), asymmetric 24-bit `- 128` S2C frames, all 76 `Controller.java` C2S opcodes (`cmd 42` fallthrough fixed), longest-prefix admin parser (`"upp"` before `"up"`), split Tokio TCP session tasks |
| `crates/nro-db` | `database/*` (7 DAOs), `Manager.loadDatabase`, 41 SQL tables | Embedded pure-Rust `fjall 3.1` + `postcard` binary storage across 16 keyspaces, zero-MySQL streaming `nro-import-sql` importer, Argon2id transparent password upgrade, striped CAS `vnd`/giftcode guards, O(100) prefix leaderboards |
| `crates/nro-player` | `player/*` (27), `skill/*`, `task/*`, `clan/*`, `Bot/*`, `radar/*`, `badges/*` | 64-bit `NPoint` stat engine (`i32::MAX` wire clamp), `SetClothes` SKH/Seal/Tier sets, `Pet`/`NewPet`/`Fusion`, all 25 skills + Mafuba/SuperKame, `TaskService.loadTask()` IP DRM removed |
| `crates/nro-economy` | `combine/*` (32), `shop/*` (12), `shop_ky_gui/*` (3), `npc_list/*` (59), `UseItem`, `Trade` | `ShopTab.java` IP DRM removed, Gold->`Thỏi vàng` (`457`) 33.3M× consign dupe fixed, `[slot, slot]` combine aliasing blocked, `Trade.java` `0`-qty & `570` (`Rương Gỗ`) escrow hardened, all 59 NPCs |
| `crates/nro-boss` | `boss/*` (159), `boss_con_duong_ran_doc/*` (3), `mob/*` (5), `mob_bigboss/*` (9), `RewardService` | Data-driven 16-group `BossGroupManager` + `data/bosses_registry.json` (128 profiles), `DrKore.injured(long)` fix, 5 seasonal boss managers wired, `rewardCapsuleTet` inverted RNG bounds fixed |
| `crates/nro-world` | `map/*` (17), `matches/*` (22), `services_dungeon/*` (10), `shenron/*`, `event/*`, `minigame/*` | Single-threaded 50 ms `ZoneActor` (`SlotMap` + SoA slices), deadlock-free two-phase `ChangeMap` over `DashMap`, 6 dungeons, 4 tournaments + `SuperRank`, `SHENRON_1_3` implemented, `ThachDau` single gold deduction |
| `crates/nro-server` | `ServerManager`, `DataGame`, `ItemData`, `Client`, `Maintenance` | `DataGameAssets` `DashMap` binary asset cache over `SRC/data` (`-74`, `-87`, `-67`, `-66`, `66`, `-32`, `-28`, `-29`, `-24`), `clap` CLI (`--import-sql`, `--assets-dir`), live `fjall` leaderboards, 4-stage LLVM PGO replay |

## Quickstart (Zero MySQL, Zero `20.jar` Required)

1. Place `database team2026.sql` and `SRC/data` in the repository directory, then run the one-click launcher for your OS (auto-builds release binaries if needed, seeds `data/nro.fjall` on first boot, and opens TCP `14445` for `Mod Local`):
   - **Linux / WSL2 / macOS**: `./start.sh` (pass `--rebuild` to force a fresh compile)
   - **Windows (Explorer / PowerShell / CMD)**: `.\start.bat` (or double-click `start.bat`)
2. Or run the SQL importer and server binaries explicitly via Cargo:
   ```bash
   cargo run --release -p nro-db --bin nro-import-sql -- --sql "database team2026.sql" --out "data/nro.fjall" --verify
   cargo run --release -p nro-server -- --config server.toml --assets-dir SRC/data
   ```
