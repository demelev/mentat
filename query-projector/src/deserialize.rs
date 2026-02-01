// Copyright 2018 Mozilla
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

//! Helper functions for deserializing custom structs from query results.
//!
//! This module provides utilities to convert `RelResult<Binding>` into user-defined
//! structs, enabling ergonomic mapping of query results to domain types.

use core_traits::Binding;
use relresult::RelResult;
use serde;
use serde_json;

/// Errors that can occur during deserialization from `RelResult`.
#[derive(Debug)]
pub enum RelResultDeserializeError {
    /// Serialization error when converting Binding to intermediate format
    SerializationError(String),
    /// Deserialization error when converting to target type
    DeserializationError(String),
}

impl ::std::fmt::Display for RelResultDeserializeError {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        match self {
            RelResultDeserializeError::SerializationError(msg) => {
                write!(f, "Serialization error: {}", msg)
            }
            RelResultDeserializeError::DeserializationError(msg) => {
                write!(f, "Deserialization error: {}", msg)
            }
        }
    }
}

impl ::std::error::Error for RelResultDeserializeError {}

/// Converts a row (Vec<Binding>) to a user-defined struct.
///
/// This function uses serde to convert a row of bindings into any type that
/// implements `serde::de::DeserializeOwned`.
///
/// # Arguments
///
/// * `row` - A vector of bindings representing one row from a query result
///
/// # Returns
///
/// Returns `Ok(T)` if deserialization succeeds, or an error if the row cannot
/// be converted to the target type.
///
/// # Example
///
/// ```ignore
/// use core_traits::{Binding, TypedValue};
/// use mentat_query_projector::deserialize::row_to_struct;
/// use serde::Deserialize;
///
/// #[derive(Deserialize, Debug, PartialEq)]
/// struct Person {
///     id: i64,
///     name: String,
/// }
///
/// let row = vec![
///     Binding::Scalar(TypedValue::Long(42)),
///     Binding::Scalar(TypedValue::from("Alice")),
/// ];
///
/// let person: Person = row_to_struct(row).unwrap();
/// assert_eq!(person.id, 42);
/// assert_eq!(person.name, "Alice");
/// ```
pub fn row_to_struct<T>(row: Vec<Binding>) -> Result<T, RelResultDeserializeError>
where
    T: serde::de::DeserializeOwned,
{
    // First, serialize the Vec<Binding> to a serde_json::Value
    // This leverages the existing Serialize impl on Binding
    let json_value = serde_json::to_value(&row)
        .map_err(|e| RelResultDeserializeError::SerializationError(e.to_string()))?;
    
    // Then deserialize from the JSON value to the target type
    serde_json::from_value(json_value)
        .map_err(|e| RelResultDeserializeError::DeserializationError(e.to_string()))
}

/// Converts a `RelResult<Binding>` to a vector of user-defined structs.
///
/// This function iterates over all rows in the `RelResult` and deserializes each
/// row into the target type `T`.
///
/// # Arguments
///
/// * `result` - A RelResult containing query results
///
/// # Returns
///
/// Returns `Ok(Vec<T>)` containing all successfully deserialized rows, or an error
/// if any row fails to deserialize.
///
/// # Example
///
/// ```ignore
/// use core_traits::{Binding, TypedValue};
/// use mentat_query_projector::{RelResult, deserialize::rel_result_to_structs};
/// use serde::Deserialize;
///
/// #[derive(Deserialize, Debug, PartialEq)]
/// struct Person {
///     id: i64,
///     name: String,
/// }
///
/// let mut result = RelResult::empty(2);
/// result.values.push(Binding::Scalar(TypedValue::Long(1)));
/// result.values.push(Binding::Scalar(TypedValue::from("Alice")));
/// result.values.push(Binding::Scalar(TypedValue::Long(2)));
/// result.values.push(Binding::Scalar(TypedValue::from("Bob")));
///
/// let people: Vec<Person> = rel_result_to_structs(result).unwrap();
/// assert_eq!(people.len(), 2);
/// assert_eq!(people[0].name, "Alice");
/// assert_eq!(people[1].name, "Bob");
/// ```
pub fn rel_result_to_structs<T>(result: RelResult<Binding>) -> Result<Vec<T>, RelResultDeserializeError>
where
    T: serde::de::DeserializeOwned,
{
    result.into_iter()
        .map(row_to_struct)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_traits::TypedValue;

    #[derive(Debug, PartialEq, Deserialize)]
    struct TestPerson {
        id: i64,
        name: String,
    }

    #[derive(Debug, PartialEq, Deserialize)]
    struct TestProduct {
        id: i64,
        name: String,
        price: f64,
        in_stock: bool,
    }

    #[test]
    fn test_row_to_struct_simple() {
        let row = vec![
            Binding::Scalar(TypedValue::Long(42)),
            Binding::Scalar(TypedValue::from("Alice")),
        ];

        let person: TestPerson = row_to_struct(row).unwrap();
        assert_eq!(person.id, 42);
        assert_eq!(person.name, "Alice");
    }

    #[test]
    fn test_row_to_struct_complex() {
        let row = vec![
            Binding::Scalar(TypedValue::Long(1)),
            Binding::Scalar(TypedValue::from("Laptop")),
            Binding::Scalar(TypedValue::Double(999.99.into())),
            Binding::Scalar(TypedValue::Boolean(true)),
        ];

        let product: TestProduct = row_to_struct(row).unwrap();
        assert_eq!(product.id, 1);
        assert_eq!(product.name, "Laptop");
        assert_eq!(product.price, 999.99);
        assert_eq!(product.in_stock, true);
    }

    #[test]
    fn test_rel_result_to_structs() {
        let mut result = RelResult::empty(2);
        result.values.push(Binding::Scalar(TypedValue::Long(1)));
        result.values.push(Binding::Scalar(TypedValue::from("Alice")));
        result.values.push(Binding::Scalar(TypedValue::Long(2)));
        result.values.push(Binding::Scalar(TypedValue::from("Bob")));
        result.values.push(Binding::Scalar(TypedValue::Long(3)));
        result.values.push(Binding::Scalar(TypedValue::from("Charlie")));

        let people: Vec<TestPerson> = rel_result_to_structs(result).unwrap();
        assert_eq!(people.len(), 3);
        assert_eq!(people[0], TestPerson { id: 1, name: "Alice".to_string() });
        assert_eq!(people[1], TestPerson { id: 2, name: "Bob".to_string() });
        assert_eq!(people[2], TestPerson { id: 3, name: "Charlie".to_string() });
    }

    #[test]
    fn test_rel_result_to_structs_empty() {
        let result: RelResult<Binding> = RelResult::empty(2);
        let people: Vec<TestPerson> = rel_result_to_structs(result).unwrap();
        assert_eq!(people.len(), 0);
    }

    #[test]
    fn test_row_to_struct_type_mismatch() {
        // Try to deserialize a string where a number is expected
        let row = vec![
            Binding::Scalar(TypedValue::from("not a number")),
            Binding::Scalar(TypedValue::from("Alice")),
        ];

        let result: Result<TestPerson, _> = row_to_struct(row);
        assert!(result.is_err());
        match result {
            Err(RelResultDeserializeError::DeserializationError(_)) => {},
            _ => panic!("Expected DeserializationError"),
        }
    }
}
