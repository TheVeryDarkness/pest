// pest. The Elegant Parser
// Copyright (c) 2018 Dragoș Tiselice
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

use crate::error::Error;
use crate::iterators::Pairs;
use crate::iterators::TypedNode;
use crate::RuleType;

/// A trait with a single method that parses strings.
pub trait Parser<R: RuleType> {
    /// Parses a `&str` starting from `rule`.
    #[allow(clippy::perf)]
    fn parse(rule: R, input: &str) -> Result<Pairs<'_, R>, Error<R>>;
}

/// A trait with a single method that parses strings into typed concrete syntax tree.
pub trait TypedParser<R: RuleType> {
    /// Parses a `&str` into a tree starting from T.
    #[allow(clippy::perf)]
    fn parse<'i, T: TypedNode<'i, R>>(input: &'i str) -> Result<(&'i str, T), Error<R>>;
}
