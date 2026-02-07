// Copyright 2024 Mentat Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

//! Helper implementations for reading entities from the database

use std::collections::HashMap;

use core_traits::{Entid, TypedValue};

use mentat_core::Keyword;

use mentat_transaction::{InProgress, Queryable};

use mentat_transaction::query::{QueryInputs, QueryResults};

use edn::query::Variable;

use public_traits::errors::{MentatError, Result};

use std::collections::BTreeMap;

use super::EntitySchema;

/// Read all attributes for an entity from the database
/// Returns a HashMap of attribute keywords to typed values
pub fn read_entity_attributes<'a, 'c>(
    in_progress: &InProgress<'a, 'c>,
    entid: Entid,
    schema: &EntitySchema,
) -> Result<HashMap<Keyword, TypedValue>> {
    let mut results = HashMap::new();

    // For each field in the schema, query the database
    for field in &schema.fields {
        let query = format!("[:find ?v . :in ?e :where [?e {} ?v]]", field.ident);

        let var = Variable::from_valid_name("?e");
        let value = TypedValue::Ref(dbg!(entid));
        let mut values = BTreeMap::new();
        values.insert(var, value);
        let inputs = QueryInputs::with_values(values);

        match in_progress.q_once(&query, Some(inputs)) {
            Ok(query_output) => {
                match dbg!(query_output.results) {
                    QueryResults::Scalar(Some(binding)) => {
                        if let Some(typed_value) = binding.into_scalar() {
                            results.insert(field.ident.clone(), typed_value);
                        }
                    }
                    QueryResults::Scalar(None) => {
                        // Attribute not present, skip (will be handled as Option::None or error)
                    }
                    _ => {
                        return Err(MentatError::UnknownAttribute(format!(
                            "Expected scalar result for attribute {}",
                            field.ident
                        )));
                    }
                }
            }
            Err(e) => {
                // If the attribute doesn't exist in the database schema yet, that's okay
                // We'll handle missing values when constructing the entity
                if !field.optional {
                    // Only propagate errors for required fields
                    return Err(e);
                }
            }
        }
    }

    Ok(results)
}

/// Find an entity ID by a unique attribute value
pub fn find_entity_by_unique<'a, 'c>(
    in_progress: &InProgress<'a, 'c>,
    attribute: &Keyword,
    value: TypedValue,
) -> Result<Option<Entid>> {
    let query = format!("[:find ?e :in ?v :where [?e :{} ?v]]", attribute);

    let var = Variable::from_valid_name("?v");
    let mut values = BTreeMap::new();
    values.insert(var, value);
    let inputs = QueryInputs::with_values(values);

    match in_progress.q_once(&query, Some(inputs)) {
        Ok(query_output) => match query_output.results {
            QueryResults::Scalar(Some(binding)) => {
                if let Some(TypedValue::Ref(entid)) = binding.into_scalar() {
                    Ok(Some(entid))
                } else {
                    Err(MentatError::UnknownAttribute(format!(
                        "Expected entid for attribute {}",
                        attribute
                    )))
                }
            }
            QueryResults::Scalar(None) => Ok(None),
            _ => Err(MentatError::UnknownAttribute(format!(
                "Expected scalar result for attribute {}",
                attribute
            ))),
        },
        Err(e) => Err(e),
    }
}
