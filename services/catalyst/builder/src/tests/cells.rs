//! How the matrices run their cells, and their audit.
//!
//! [`in_worlds`] spreads a matrix's cells over up to [`WORLDS`] worlds, one
//! for every [`CELLS_PER_WORLD`] cells, each with its own scratch database,
//! and runs each world's cells one at a time: a cell's whole-database
//! snapshot sees only its own writes.
//!
//! With `BUILDER_MATRIX_CELLS` naming a file, every matrix cell a test
//! evaluates appends one tab-separated line to it ([`record`]): the test's
//! condition, the operation, the resource state, the caller, the mount, the
//! expected answer, the answer and the checks the cell ran. Two runs' sorted
//! files list the same lines exactly when they assert the same cells.
