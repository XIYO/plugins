# Architecture

## Product boundary

Consumers install one plugin: `sherpa@xiyo`. The plugin bundles multiple skills because routing policy and specialist safety rules change for different data sources.

```text
sherpa skill
├── apple-calendar skill  -> calctl + calmeta
├── apple-reminders skill -> RemCTL
└── message-pipeline      -> msgpipe -> kakaocli / imsg
```

There is no plugin-to-plugin dependency. Codex loads every specialist from this plugin's own `skills/` directory.

## Runtime boundaries

- `calctl` is the only Calendar/EventKit state adapter.
- `calmeta` parses and validates structured Calendar notes without accessing Calendar data.
- RemCTL owns Reminders access. Sherpa never writes to the Reminders SQLite store directly.
- `msgpipe` owns the protected raw-message archive, pending state, CCT export, and summary watermarks. Source readers remain read-only.
- Sherpa owns intent classification, bounded collection, confirmation policy, and cross-source presentation. It does not reimplement specialist parsers.

## Version boundaries

The Sherpa plugin has its own SemVer. Bundled runtime versions remain independent and are pinned in `runtime-versions.json`. This avoids pretending that a Calendar metadata parser, a Swift EventKit adapter, a message archive, and an external Reminders tool share one release lifecycle.

## Migration boundary

The original specialist plugin sources remain in the repository during the compatibility window. Sherpa preserves their executable names and data paths. CI compares canonical runtime source copies so a security or compatibility fix cannot silently land in only one distribution path.
