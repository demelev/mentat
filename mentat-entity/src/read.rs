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

use core_traits::{Entid, KnownEntid, StructuredMap, TypedValue};

use edn::entities::EntidOrIdent;
use mentat_core::{HasSchema, Keyword};

use mentat_transaction::{InProgress, InProgressRead, Pullable, Queryable};

use mentat_transaction::query::{QueryInputs, QueryResults};

use edn::query::Variable;

use public_traits::errors::{MentatError, Result};

use std::collections::BTreeMap;

use crate::MentatEntityError;

use super::EntitySchema;

/// Read all attributes for an entity from the database
/// Returns a HashMap of attribute keywords to typed values
pub fn read_entity_attributes<'a, 'c>(
    in_progress: &mut InProgressRead<'a, 'c>,
    entid: Entid,
    schema: &EntitySchema,
) -> Result<StructuredMap> {
    // For each field in the schema, query the database
    let attr_iter = schema.fields.iter().map(|f| {
        in_progress
            .get_entid(&f.ident)
            .ok_or(MentatError::EntityError(format!(
                "Database missing attribute {}",
                &f.ident
            )))
    });
    let mut attributes = vec![];
    for attr in attr_iter {
        match attr {
            Ok(attr) => attributes.push(attr.0),
            Err(e) => {
                // If the attribute doesn't exist in the database schema yet, that's okay
                // We'll handle missing values when constructing the entity
                // if !schema.fields.iter().any(|f| f.ident == attr.unwrap()) {
                // Only propagate errors for required fields
                return Err(e.into());
                // }
            }
        }
    }

    let mut map = in_progress.pull_attributes_for_entity(entid, attributes)?;
    if let Some(field) = schema
        .fields
        .iter()
        .find(|f| f.ident.components() == ("db", "id"))
    {
        map.insert(field.ident.clone(), TypedValue::Long(entid));
    }
    Ok(map)
}

/// Find an entity ID by a unique attribute value
pub fn find_entity_by_unique<'a, 'c>(
    in_progress: &mut InProgressRead<'a, 'c>,
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
