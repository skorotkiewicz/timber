# timber

Decode `tree` text output back to files - recreate directory structures from `tree` dumps.

Also encodes directories to JSON (with optional file contents).

## Build

```sh
cargo build --release
```

## Usage

**Encode** a directory tree to JSON:

```sh
timber encode /your/path > tree.json
```

Limit depth:

```sh
timber encode /your/path --depth 3 > tree.json
```

Exclude files and directories:

```sh
timber encode /your/path --exclude .git --exclude target --exclude node_modules > tree.json
# Or use short flag:
timber encode /your/path -e .git -e target -e Cargo.lock > tree.json
```

Omit file contents (structure only):

```sh
timber encode /your/path --no-content > tree.json
# Or use short flag:
timber encode /your/path -n > tree.json
```

**Decode** JSON back to files:

```sh
timber decode tree.json ./output
```

**Decode** from a plain `tree` text dump:

```sh
tree /path/to/project > tree.txt
timber decode tree.txt ./output
```

**Decode** from `tree -J` JSON output:

```sh
tree -J /path/to/project > tree.json
timber decode tree.json ./output
```

## Format

- `type` - `"dir"` or `"file"`
- `content` - present only for files, holds file contents (omitted when using `--no-content` / `-n`)
- `children` - present only for directories
