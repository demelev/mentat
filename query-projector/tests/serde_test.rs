// Copyright 2018 Mozilla
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

extern crate core_traits;
extern crate mentat_query_projector;
extern crate serde_json;

use core_traits::{
    Binding,
    TypedValue,
};

use mentat_query_projector::{
    QueryResults,
    RelResult,
};

#[test]
fn test_query_results_scalar_serialize() {
    let results = QueryResults::Scalar(Some(Binding::Scalar(TypedValue::Long(42))));
    let json = serde_json::to_string(&results).expect("serialization should succeed");
    let deserialized: QueryResults = serde_json::from_str(&json).expect("deserialization should succeed");
    assert_eq!(results, deserialized);
}

#[test]
fn test_query_results_tuple_serialize() {
    let results = QueryResults::Tuple(Some(vec![
        Binding::Scalar(TypedValue::Long(1)),
        Binding::Scalar(TypedValue::Boolean(true)),
    ]));
    let json = serde_json::to_string(&results).expect("serialization should succeed");
    let deserialized: QueryResults = serde_json::from_str(&json).expect("deserialization should succeed");
    assert_eq!(results, deserialized);
}

#[test]
fn test_query_results_coll_serialize() {
    let results = QueryResults::Coll(vec![
        Binding::Scalar(TypedValue::Long(1)),
        Binding::Scalar(TypedValue::Long(2)),
        Binding::Scalar(TypedValue::Long(3)),
    ]);
    let json = serde_json::to_string(&results).expect("serialization should succeed");
    let deserialized: QueryResults = serde_json::from_str(&json).expect("deserialization should succeed");
    assert_eq!(results, deserialized);
}

#[test]
fn test_rel_result_serialize() {
    let mut rel_result = RelResult::empty(2);
    rel_result.values.push(Binding::Scalar(TypedValue::Long(1)));
    rel_result.values.push(Binding::Scalar(TypedValue::from("Alice")));
    rel_result.values.push(Binding::Scalar(TypedValue::Long(2)));
    rel_result.values.push(Binding::Scalar(TypedValue::from("Bob")));
    
    let json = serde_json::to_string(&rel_result).expect("serialization should succeed");
    let deserialized: RelResult<Binding> = serde_json::from_str(&json).expect("deserialization should succeed");
    assert_eq!(rel_result, deserialized);
}

#[test]
fn test_query_results_rel_serialize() {
    let mut rel_result = RelResult::empty(2);
    rel_result.values.push(Binding::Scalar(TypedValue::Long(1)));
    rel_result.values.push(Binding::Scalar(TypedValue::from("Alice")));
    
    let results = QueryResults::Rel(rel_result);
    let json = serde_json::to_string(&results).expect("serialization should succeed");
    let deserialized: QueryResults = serde_json::from_str(&json).expect("deserialization should succeed");
    assert_eq!(results, deserialized);
}
