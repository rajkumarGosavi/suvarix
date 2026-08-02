//! Filed income-tax returns (ITR-2). Storage + reporting only: the PDF itself is
//! parsed in the frontend (`src/utils/itrParser.ts`) and arrives here as a struct
//! the user has already reviewed, so nothing in this module reads PDFs.
pub mod commands;
