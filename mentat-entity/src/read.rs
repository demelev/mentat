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

use core_traits::{
    Binding,
    Entid,
    TypedValue,
};

use mentat_core::{
    Keyword,
    Schema,
};

use mentat_transaction::{
    InProgress,
    query::{
        InProgressRead,
        QueryInputs,
        QueryResults,
    },
};

use edn::query::{
    Variable,
};

use public_traits::errors::{
    MentatError,
    Result,
};

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
        let query = format!(
            "[:find ?v . :in $ ?e :where [?e :{} ?v]]",
            field.ident
        );
        
        let mut inputs = QueryInputs::new();
        inputs = inputs.bind(Variable::from_valid_name("?e"), TypedValue::Ref(entid));
        
        match in_progress.q_once(&query, inputs) {
            Ok(query_output) => {
                match query_output.results {
                    QueryResults::Scalar(Some(binding)) => {
                        if let Some(typed_value) = binding.into_typed_value() {
                            results.insert(field.ident.clone(), typed_value);
                        }
                    }
                    QueryResults::Scalar(None) => {
                        // Attribute not present, skip (will be handled as Option::None or error)
                    }
                    _ => {
                        return Err(MentatError::UnexpectedResultsType(
                            "Expected scalar result".to_string(),
                        ).into());
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
    let query = format!(
        "[:find ?e . :in $ ?v :where [?e :{} ?v]]",
        attribute
    );
    
    let mut inputs = QueryInputs::new();
    inputs = inputs.bind(Variable::from_valid_name("?v"), value);
    
    match in_progress.q_once(&query, inputs) {
        Ok(query_output) => {
            match query_output.results {
                QueryResults::Scalar(Some(binding)) => {
                    if let Some(TypedValue::Ref(entid)) = binding.into_typed_value() {
                        Ok(Some(entid))
                    } else {
                        Err(MentatError::UnexpectedResultsType(
                            "Expected entid".to_string(),
                        ).into())
                    }
                }
                QueryResults::Scalar(None) => Ok(None),
                _ => {
                    Err(MentatError::UnexpectedResultsType(
                        "Expected scalar result".to_string(),
                    ).into())
                }
            }
        }
        Err(e) => Err(e),
    }
}
