# indexer

Indexer for offline analysis of on-chain data.

Run with:
  - `cargo run --release --bin dindexer -- --help`
  - `cargo run --release --bin ridx -- --help` (Currently needed for most of the pipleline. Edit: No longer needed)

Sequences:
  - Run ridx to build minimal (block) index with cli indexer connecting only to defid
  - Run ridx with txindexer to index transactions
  - Run grapher, etc on rocksdb
  - Build sqlite indexes from rocksdb indexes

TODO:
  - [Done] Build the sqlite indexes directly from CLI and remove rocksdb
  - Graph reducer and more graph stuff.

### Notes

- This is code written in a rush for specific requirements. Has all the pieces, but will likely require cleanup and refactoring to be generalized.
