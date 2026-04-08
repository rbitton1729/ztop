# zftop

A terminal-based dashboard for the Zettabyte File System, in the spirit of `htop`.

## Status

**v0.1 — proof of concept.** Right now zftop does exactly one thing: it shows you, live, how much memory your ARC is using and what's inside it. That's it. No pools view, no datasets, no snapshots, no SMART. Those are coming in later versions.

The reason zftop exists is that the existing tools each give you one slice of the picture — `zpool status`, `arc_summary`, `zfs list`, `smartctl` — and you end up running four commands and holding the whole thing in your head. zftop is the dashboard that fuses them. v0.1 is the first slice.

## What v0.1 shows

A single screen that refreshes once a second:

- **ARC size** — current size against `c_max`, as a bar, so you can see at a glance whether the ARC is near its ceiling.
- **Breakdown** — MFU data, MRU data, metadata, and other, both in bytes and as a percentage of the total.
- **Hit ratio** — overall, demand, and prefetch.
- **Throughput** — hits and misses per second.

All of it comes from `/proc/spl/kstat/zfs/arcstats`. No subprocesses, no parsing of human-formatted output, no surprises.

## Install

### Arch Linux (AUR)

```
yay -S zftop
```

Or with any AUR helper. The package installs the binary as `zftop`.

### From source

```
git clone https://git.skylantix.com/rbitton/zftop.git
cd zftop
cargo build --release
sudo install -Dm755 target/release/zftop /usr/bin/zftop
```

### FreeBSD

Same recipe — `pkg install rust && cargo build --release && install -m 755 target/release/zftop /usr/local/bin/zftop`. zftop reads ZFS state via `sysctl kstat.zfs.misc.arcstats.*` and memory via `sysctl vm.stats.vm.* hw.physmem hw.pagesize`, so it works out of the box on any FreeBSD with OpenZFS — vanilla FreeBSD, TrueNAS, pfSense, anything. The `--source` and `--meminfo` flags are Linux-only and ignored on FreeBSD.

### Prebuilt binary

Binaries are attached to every [release](https://git.skylantix.com/rbitton/zftop/-/releases):

- `zftop-linux-amd64` — Linux x86_64 (static musl, no runtime deps)
- `zftop-linux-arm64` — Linux aarch64 (static musl — Graviton, Ampere Altra, Pi 4/5)
- `zftop-freebsd-amd64` — FreeBSD amd64 (built on FreeBSD 15, dynamically links against system libc)

Download the one for your platform, then:

```
chmod +x zftop-linux-amd64
sudo mv zftop-linux-amd64 /usr/bin/zftop
```

(On FreeBSD, the conventional install path is `/usr/local/bin/zftop`.)

## Usage

```
zftop                    # default: poll every 1s
zftop -n 500             # poll every 500ms
zftop --interval 2000    # poll every 2 seconds
zftop --help             # show all options
```

## Controls

| Key | Action |
|-----|--------|
| `q` / `Ctrl+C` | quit |
| `r` | force refresh |

That's the whole interface in v0.1.

## Requirements

- Linux with OpenZFS installed (the `/proc/spl/kstat/zfs/arcstats` file must exist)
- A terminal that supports ANSI colors and box-drawing characters, which is to say any terminal made in the last 30 years

No runtime dependencies beyond the kernel module being loaded. zftop is a single static binary.

## Roadmap

zftop is a *finishable* project. ZFS is stable, the surface area we care about isn't growing, and once the dashboard shows everything worth seeing there's no v3.0 plugin system to chase. The plan is to ship a few focused versions and then stop.

- **v0.1** — ARC memory visualization (this release)
- **v0.2** — pools view: capacity, fragmentation, health, vdev tree, scrub status
- **v0.3** — datasets view: usage, compression ratios, sortable and filterable
- **v0.4** — snapshots view, with awareness of Sanoid retention classes
- **v0.5** — SMART health joined to vdev members on the pools view
- **v1.0** — Remote/Fleet mode — use ssh either independently or with Ansible to monitor many machines at once.

## Why Rust

Because I wanted to see Rust in action on a project I deeply understand. Practically: a static musl binary with no runtime dependencies is the right shape for a system tool that lives on every host you manage, and Rust + ratatui is the cleanest path to that. If this had been Python with Textual it would've been faster to write and harder to ship.

## License

MIT. See `LICENSE`.
