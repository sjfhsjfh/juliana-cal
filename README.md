# juliana-cal

A simple 🦀️ `.ics` feed generator.

Build a subscribable iCalendar feed from a TOML manifest.

## Usage

```sh
juliana-cal new deadlines.toml
# edit deadlines.toml
juliana-cal check deadlines.toml
juliana-cal build deadlines.toml -o deadlines.ics
```

`new` writes a starter manifest with a versioned JSON Schema URL.

`serve` is behind the `server` feature and is not included by default:

```sh
cargo run --features server -- serve ./feeds
```

## Apple Calendar

When adding the feed as a subscription, uncheck **Remove Alarms**. Otherwise
Apple Calendar discards the `VALARM` reminders.

## License

MIT
