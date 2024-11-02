# dindexer

Indexer for offline analysis of on-chain data.

## Notes

- Build a full index with only the blockchain node as the source.
  - Supports building rockdb datastore or SQLite store.
- Amends additional data from the source of truth (node consensus logs) to amend additional data like ICX
- Tools to explore the data and generate various different graphs of the large data set.

## Usage

```
Usage: dindexer [OPTIONS] <COMMAND>

Commands:
  index
          Index from cli sqlite db
  dotreduce
          Reduce dot graph files
  icxanalyze
          Analyze ICX claims and every address involved in the way up until the swap of the claims
  icxseq
          Output the full ICX sequence chain
  graph
          Construct the full graph and output it to a file so the graph can loaded in memory and reused directly
  graphwalk
          Load and explore full graph
  graphdot
          Load the full graph, condense it and output dot files
  spath
          Find shortest path between 2 addresses or a list of given addresses
  gpath
          Find all paths with exclusions
  help
          Print this message or the help of the given subcommand(s)

Options:
  -v, --verbosity...
          Can be called multiple times to increase level. (0-4).

          0: Error
          1: Warn
          2: Info
          3: Debug
          4: Trace

          Minimum might be pulled higher.

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

```

See help for each command for more information.
