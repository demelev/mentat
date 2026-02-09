// Copyright 2024 Mentat Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

//! Example demonstrating EntityView and EntityPatch derive macros
//!
//! This example shows how to use the new EntityView and EntityPatch macros
//! to define entity views and patches for Mentat database operations.

use mentat_entity::{
    EntityId, EntityPatch, EntityView, EntityViewSpec, FieldKind, ManyPatch, Patch, TxOp,
};

// ============================================================================
// Example 1: Person and Car with Backref
// ============================================================================

/// A view of a person entity
#[derive(EntityView, Debug)]
#[entity(ns = "person")]
struct PersonView {
    /// Entity ID - maps to :db/id
    #[attr(":db/id")]
    id: i64,

    /// Person's name - maps to :person/name
    name: String,

    /// Cars owned by this person - backref from :car/owner
    /// This is a reverse reference: we're looking for cars that reference this person
    #[backref(attr = ":car/owner")]
    cars: Vec<CarView>,
}

/// A view of a car entity
#[derive(EntityView, Debug)]
#[entity(ns = "car")]
struct CarView {
    #[attr(":db/id")]
    id: i64,

    model: String,

    /// Forward reference to the owner (a person entity)
    /// Note: Use `fref` instead of `ref` to avoid keyword conflict
    #[fref(attr = ":car/owner")]
    owner: EntityId,
}

// ============================================================================
// Example 2: Order with Patch Operations
// ============================================================================

#[derive(Clone, Debug, PartialEq)]
enum OrderStatus {
    Pending,
    Paid,
    Shipped,
}

/// Simple conversion for OrderStatus to TypedValue
impl From<OrderStatus> for mentat_entity::core_traits::TypedValue {
    fn from(status: OrderStatus) -> Self {
        use mentat_entity::core_traits::TypedValue;
        use mentat_entity::mentat_core::Keyword;

        let keyword = match status {
            OrderStatus::Pending => Keyword::namespaced("status", "pending"),
            OrderStatus::Paid => Keyword::namespaced("status", "paid"),
            OrderStatus::Shipped => Keyword::namespaced("status", "shipped"),
        };
        TypedValue::Keyword(keyword.into())
    }
}

/// A patch for updating order entities
#[derive(EntityPatch)]
#[entity(ns = "order")]
struct OrderPatch {
    /// Required: the entity ID of the order to update
    #[entity_id]
    id: EntityId,

    /// Update the order status
    /// - Patch::NoChange: don't modify
    /// - Patch::Set(value): set to new value
    /// - Patch::Unset: remove the attribute
    status: Patch<OrderStatus>,

    /// Update customer reference
    #[attr(":order/customer")]
    customer: Patch<EntityId>,

    /// Modify tags (multi-valued attribute)
    /// - add: values to add
    /// - remove: values to remove
    /// - clear: if true, remove all existing values first
    tags: ManyPatch<String>,
}

// ============================================================================
// Example 3: Product with Default Namespace
// ============================================================================

/// Product view - namespace defaults to "product_view" (snake_case of struct name)
#[derive(EntityView)]
struct ProductView {
    #[attr(":db/id")]
    id: i64,

    /// Maps to :product_view/product_name
    product_name: String,

    /// Maps to :product_view/price
    price: Option<i64>,

    /// Maps to :product_view/category (multi-valued)
    category: Vec<String>,
}

// ============================================================================
// Usage Examples
// ============================================================================

fn main() {
    println!("=== EntityView Example ===\n");

    // Inspect PersonView metadata
    println!("PersonView namespace: {}", PersonView::NS);
    println!("PersonView fields:");
    for field in PersonView::FIELDS {
        println!("  - {} -> {}", field.rust_name, field.attr);
        println!("    Kind: {:?}", field.kind);
        println!("    Many: {}", field.cardinality_many);
    }

    println!("\n=== EntityPatch Example ===\n");

    // Create a patch to update an order
    let patch = OrderPatch {
        id: EntityId::Entid(100),
        status: Patch::Set(OrderStatus::Paid),
        customer: Patch::NoChange,
        tags: ManyPatch {
            add: vec!["premium".to_string(), "express".to_string()],
            remove: vec!["basic".to_string()],
            clear: false,
        },
    };

    // Convert patch to transaction operations
    let ops = patch.to_tx();
    println!("Generated {} transaction operations:", ops.len());
    for (i, op) in ops.iter().enumerate() {
        match op {
            TxOp::Assert { e, a, v: _ } => {
                println!("  {}. Assert {:?} {}", i + 1, e, a);
            }
            TxOp::Retract { e, a, v: _ } => {
                println!("  {}. Retract {:?} {}", i + 1, e, a);
            }
            TxOp::RetractAttr { e, a } => {
                println!("  {}. RetractAttr {:?} {}", i + 1, e, a);
            }
        }
    }

    println!("\n=== Default Namespace Example ===\n");

    println!("ProductView namespace: {}", ProductView::NS);
    println!("First field attribute: {}", ProductView::FIELDS[1].attr);

    println!("\n=== Reference Types ===\n");

    // Check field kinds
    for field in PersonView::FIELDS {
        if matches!(field.kind, FieldKind::Backref { .. }) {
            println!("Found backref field: {} ({})", field.rust_name, field.attr);
        }
    }

    for field in CarView::FIELDS {
        if matches!(field.kind, FieldKind::Ref { .. }) {
            println!("Found ref field: {} ({})", field.rust_name, field.attr);
        }
    }

    println!("\nExample completed successfully!");
}
