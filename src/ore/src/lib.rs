// Copyright Materialize, Inc. and contributors. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License in the LICENSE file at the
// root of this repository, or online at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Internal utility libraries for Materialize.
//!
//! **ore** (_n_): the raw material from which more valuable materials are extracted.
//! Modules are included in this crate when they are broadly useful but too
//! small to warrant their own crate.

#![warn(missing_docs, missing_debug_implementations)]
#![cfg_attr(nightly_doc_features, feature(doc_cfg))]

#[cfg_attr(nightly_doc_features, doc(cfg(feature = "test")))]
#[cfg(feature = "test")]
pub mod assert;
pub mod bits;
#[cfg_attr(nightly_doc_features, doc(cfg(feature = "bytes_")))]
#[cfg(feature = "bytes_")]
pub mod bytes;
pub mod cast;
#[cfg_attr(nightly_doc_features, doc(cfg(feature = "async")))]
#[cfg(feature = "async")]
pub mod channel;
#[cfg_attr(nightly_doc_features, doc(cfg(feature = "cli")))]
#[cfg(feature = "cli")]
pub mod cli;
pub mod codegen;
pub mod collections;
pub mod env;
pub mod error;
pub mod fmt;
#[cfg_attr(nightly_doc_features, doc(cfg(feature = "async")))]
#[cfg(feature = "async")]
pub mod future;
pub mod graph;
pub mod hash;
pub mod hint;
#[cfg(feature = "id_gen")]
pub mod id_gen;
pub mod iter;
pub mod lex;
#[cfg_attr(
    nightly_doc_features,
    doc(cfg(all(feature = "bytes_", feature = "region")))
)]
#[cfg(all(feature = "bytes_", feature = "region"))]
pub mod lgbytes;
#[cfg_attr(nightly_doc_features, doc(cfg(feature = "metrics")))]
#[cfg(feature = "metrics")]
pub mod metrics;
#[cfg_attr(nightly_doc_features, doc(cfg(feature = "network")))]
#[cfg(feature = "network")]
pub mod netio;
pub mod now;
pub mod num;
pub mod option;
pub mod panic;
pub mod path;
pub mod permutations;
pub mod process;
#[cfg(feature = "region")]
pub mod region;
#[cfg_attr(nightly_doc_features, doc(cfg(feature = "process")))]
pub mod result;
#[cfg_attr(nightly_doc_features, doc(cfg(feature = "async")))]
#[cfg(feature = "async")]
pub mod retry;
pub mod serde;
#[cfg_attr(nightly_doc_features, doc(cfg(feature = "stack")))]
#[cfg(feature = "stack")]
pub mod stack;
pub mod stats;
pub mod str;
#[cfg_attr(nightly_doc_features, doc(cfg(feature = "async")))]
#[cfg(feature = "async")]
pub mod task;
#[cfg_attr(nightly_doc_features, doc(cfg(any(test, feature = "test"))))]
#[cfg(any(test, feature = "test"))]
pub mod test;
pub mod thread;
#[cfg_attr(nightly_doc_features, doc(cfg(feature = "tracing_")))]
#[cfg(feature = "tracing_")]
pub mod tracing;
pub mod vec;

pub use mz_test_macro::test;

#[doc(hidden)]
pub mod __private {
    #[cfg(feature = "tracing_")]
    pub use tracing;
}
