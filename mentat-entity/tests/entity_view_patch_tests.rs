// Copyright 2024 Mentat Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

//! Tests for EntityView and EntityPatch derive macros

use mentat_entity::{
    EntityId, EntityPatch, EntityView, EntityViewSpec, FieldKind, FieldSpec, ManyPatch, Patch,
    TxOp,
};

// ============================================================================
// Test 1: PersonView and CarView with backref
// ============================================================================

#[derive(EntityView, Debug)]
#[entity(ns = "person")]
struct PersonView {
    #[attr(":db/id")]
    id: i64,
    name: String,
    #[backref(attr = ":car/owner")]
    car: Option<CarView>,
}

#[derive(EntityView, Debug)]
#[entity(ns = "car")]
struct CarView {
    #[attr(":db/id")]
    id: i64,
    model: String,
    #[r#ref(attr = ":car/owner")]
    owner: EntityId,
}

#[test]
fn test_person_view_spec() {
    assert_eq!(PersonView::NS, "person");
    assert_eq!(PersonView::FIELDS.len(), 3);

    // Check id field
    let id_field = &PersonView::FIELDS[0];
    assert_eq!(id_field.rust_name, "id");
    assert_eq!(id_field.attr, ":db/id");
    assert!(matches!(id_field.kind, FieldKind::Scalar));
    assert!(!id_field.cardinality_many);

    // Check name field
    let name_field = &PersonView::FIELDS[1];
    assert_eq!(name_field.rust_name, "name");
    assert_eq!(name_field.attr, ":person/name");
    assert!(matches!(name_field.kind, FieldKind::Scalar));

    // Check car field (backref)
    let car_field = &PersonView::FIELDS[2];
    assert_eq!(car_field.rust_name, "car");
    assert_eq!(car_field.attr, ":car/owner"); // forward ident for backref
    assert!(matches!(
        car_field.kind,
        FieldKind::Backref { nested: "CarView" }
    ));
    assert!(!car_field.cardinality_many);
}

#[test]
fn test_car_view_spec() {
    assert_eq!(CarView::NS, "car");
    assert_eq!(CarView::FIELDS.len(), 3);

    // Check owner field (forward ref)
    let owner_field = &CarView::FIELDS[2];
    assert_eq!(owner_field.rust_name, "owner");
    assert_eq!(owner_field.attr, ":car/owner");
    
    // Debug output
    println!("owner_field.kind = {:?}", owner_field.kind);
    
    // The nested type should be the base type name "EntityId"
    match &owner_field.kind {
        FieldKind::Ref { nested } => {
            assert_eq!(*nested, "EntityId", "Expected nested type to be EntityId, got {}", nested);
        }
        _ => panic!("Expected FieldKind::Ref, got {:?}", owner_field.kind),
    }
}

// ============================================================================
// Test 2: OrderPatch with Patch and ManyPatch
// ============================================================================

#[derive(Clone, Debug, PartialEq)]
enum OrderStatus {
    Pending,
    Paid,
    Shipped,
}

// Simple conversion for testing
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

#[derive(EntityPatch)]
#[entity(ns = "order")]
struct OrderPatch {
    #[entity_id]
    id: EntityId,
    status: Patch<OrderStatus>,
    #[attr(":order/customer")]
    customer: Patch<EntityId>,
    tags: ManyPatch<String>,
}

#[test]
fn test_order_patch_to_tx() {
    let patch = OrderPatch {
        id: EntityId::Entid(100),
        status: Patch::Set(OrderStatus::Paid),
        customer: Patch::NoChange,
        tags: ManyPatch {
            add: vec!["vip".to_string()],
            remove: vec![],
            clear: false,
        },
    };

    let ops = patch.to_tx();
    
    // Should have 2 ops: one for status Set, one for tags add
    assert_eq!(ops.len(), 2);

    // Check status op
    match &ops[0] {
        TxOp::Assert { e, a, v } => {
            assert_eq!(e, &EntityId::Entid(100));
            assert_eq!(*a, ":order/status");
            // Value check would require more setup
        }
        _ => panic!("Expected Assert for status"),
    }

    // Check tags op
    match &ops[1] {
        TxOp::Assert { e, a, v } => {
            assert_eq!(e, &EntityId::Entid(100));
            assert_eq!(*a, ":order/tags");
        }
        _ => panic!("Expected Assert for tags"),
    }
}

#[test]
fn test_order_patch_unset() {
    let patch = OrderPatch {
        id: EntityId::Entid(100),
        status: Patch::Unset,
        customer: Patch::NoChange,
        tags: ManyPatch::new(),
    };

    let ops = patch.to_tx();
    
    // Should have 1 op: RetractAttr for status
    assert_eq!(ops.len(), 1);

    match &ops[0] {
        TxOp::RetractAttr { e, a } => {
            assert_eq!(e, &EntityId::Entid(100));
            assert_eq!(*a, ":order/status");
        }
        _ => panic!("Expected RetractAttr for status"),
    }
}

#[test]
fn test_many_patch_clear() {
    let patch = OrderPatch {
        id: EntityId::Entid(100),
        status: Patch::NoChange,
        customer: Patch::NoChange,
        tags: ManyPatch {
            add: vec!["premium".to_string()],
            remove: vec![],
            clear: true,
        },
    };

    let ops = patch.to_tx();
    
    // Should have 2 ops: RetractAttr for clear, then Assert for add
    assert_eq!(ops.len(), 2);

    match &ops[0] {
        TxOp::RetractAttr { e, a } => {
            assert_eq!(e, &EntityId::Entid(100));
            assert_eq!(*a, ":order/tags");
        }
        _ => panic!("Expected RetractAttr for tags clear"),
    }

    match &ops[1] {
        TxOp::Assert { e, a, v } => {
            assert_eq!(e, &EntityId::Entid(100));
            assert_eq!(*a, ":order/tags");
        }
        _ => panic!("Expected Assert for tags add"),
    }
}

#[test]
fn test_many_patch_remove() {
    let patch = OrderPatch {
        id: EntityId::Entid(100),
        status: Patch::NoChange,
        customer: Patch::NoChange,
        tags: ManyPatch {
            add: vec![],
            remove: vec!["old-tag".to_string()],
            clear: false,
        },
    };

    let ops = patch.to_tx();
    
    // Should have 1 op: Retract for remove
    assert_eq!(ops.len(), 1);

    match &ops[0] {
        TxOp::Retract { e, a, v } => {
            assert_eq!(e, &EntityId::Entid(100));
            assert_eq!(*a, ":order/tags");
        }
        _ => panic!("Expected Retract for tags remove"),
    }
}

// ============================================================================
// Test 3: Default namespace (snake_case of struct name)
// ============================================================================

#[derive(EntityView)]
struct ProductView {
    #[attr(":db/id")]
    id: i64,
    product_name: String,
    price: Option<i64>,
}

#[test]
fn test_default_namespace() {
    // Should default to "product_view" (snake_case of "ProductView")
    assert_eq!(ProductView::NS, "product_view");
    
    // product_name should be ":product_view/product_name"
    let name_field = &ProductView::FIELDS[1];
    assert_eq!(name_field.attr, ":product_view/product_name");
}

// ============================================================================
// Test 4: Cardinality many with Vec
// ============================================================================

#[derive(EntityView)]
#[entity(ns = "article")]
struct ArticleView {
    #[attr(":db/id")]
    id: i64,
    title: String,
    tags: Vec<String>,
}

#[test]
fn test_cardinality_many() {
    let tags_field = &ArticleView::FIELDS[2];
    assert_eq!(tags_field.rust_name, "tags");
    assert!(tags_field.cardinality_many);
    assert!(matches!(tags_field.kind, FieldKind::Scalar));
}

// ============================================================================
// Test 5: Scalar fields with default attributes
// ============================================================================

#[derive(EntityPatch)]
struct UserPatch {
    #[entity_id]
    id: EntityId,
    email: Patch<String>,
    age: Patch<i64>,
    active: Patch<bool>,
}

#[test]
fn test_user_patch_default_attrs() {
    let patch = UserPatch {
        id: EntityId::Entid(1),
        email: Patch::Set("test@example.com".to_string()),
        age: Patch::Set(30),
        active: Patch::Set(true),
    };

    let ops = patch.to_tx();
    assert_eq!(ops.len(), 3);

    // Check that default namespace "user_patch" is used
    let attrs: Vec<&str> = ops.iter().map(|op| match op {
        TxOp::Assert { a, .. } => *a,
        _ => panic!("Expected Assert"),
    }).collect();

    assert!(attrs.contains(&":user_patch/email"));
    assert!(attrs.contains(&":user_patch/age"));
    assert!(attrs.contains(&":user_patch/active"));
}
