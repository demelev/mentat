# Техническое задание: Codegen EntityView/EntityPatch + Pull/Backref для Mentat/Datomic‑style слоя (Rust)

## 1. Контекст и цель

Проект: **Mentat rust datomic database** (Datomic/Mentat‑подобная модель: факты/датомы, immutable snapshots, Datalog‑запросы, pull‑паттерны).  
Нужно добавить фичу: **кодогенерация (derive‑макросы)** для описания чтения/записи entity через Rust‑структуры:

- `EntityView` — декларация **проекции для чтения** (pull‑shape + метаданные refs/backrefs).
- `EntityPatch` — декларация **патча для записи** (Patch/ManyPatch → TxOps).
- (опционально в будущем) `EntityTx` — команды создания/upsert (tempid/lookupref).

Результат должен дать разработчику возможность описывать структуру данных в Rust и автоматически получать:
1) pull‑pattern/metadata для загрузки графа с контролем вложенности и backrefs;
2) генерацию транзакционных операций (assert/retract/retract-attr/ensure).

## 2. Область работ (Scope)

### 2.1. Must have (MVP)
1) Crate `entity_codegen` (proc-macro) с derive:
   - `#[derive(EntityView)]`
   - `#[derive(EntityPatch)]`
2) Базовые атрибуты аннотаций:
   - `#[entity(ns="person")]` — namespace по умолчанию на struct
   - default namespace: `snake_case(struct_name)` если `ns` не указан
   - `#[attr(":custom/ident")]` — override атрибута на поле
   - `#[entity_id]` — поле идентификатора сущности (Entid/EntityId)
   - `#[ref(attr=":x/y")]` — forward ref
   - `#[backref(attr=":x/y")]` — reverse ref (читается через `:x/_y` в pull или через отдельную фазу в репозитории)
3) Типы патча:
   - `Patch<T> = NoChange | Set(T) | Unset`
   - `ManyPatch<T> { add: Vec<T>, remove: Vec<T>, clear: bool }`
4) Генерация `to_tx()` для `EntityPatch`:
   - `Set` → `Assert`
   - `Unset` → `RetractAttr`
   - `ManyPatch.add` → `Assert` для каждого
   - `ManyPatch.remove` → `Retract` для каждого
   - `ManyPatch.clear` → `RetractAttr`
5) Генерация метаданных для `EntityView`:
   - список полей/атрибутов
   - тип поля (scalar/ref/backref)
   - nested view type для refs/backrefs
   - кардинальность (one/many) по типу поля: `T`/`Option<T>` = one, `Vec<T>` = many
6) Пример интеграции в проект: демонстрационный модуль/тесты с `PersonView` + `CarView`, включая backref `:car/owner`.

### 2.2. Nice to have (после MVP)
- Профили view (`profile="shallow"|"with_car"`) или разные структуры, выбор через API репозитория.
- `Ensure`/CAS‑предикаты (optimistic concurrency) в `EntityPatch` через `#[ensure(...)]`.
- Компоненты (`component=true`) и каскадные операции.
- LookupRef/TempId полноценные сценарии upsert/create.
- Генерация EDN pull‑pattern напрямую (если используется EDN в проекте).

## 3. Термины

- **Entid**: внутренний id сущности (обычно i64).
- **EntityId**: универсальная ссылка на сущность: `Entid | LookupRef | TempId`.
- **Ref**: атрибут‑ссылка на другую сущность.
- **Backref**: обратная ссылка (в Datomic — `:_ns/attr`) в Mentat :ns/_attr.
- **Pull**: описание формы данных (shape), не логика.
- **TxOp**: транзакционная операция (assert/retract/...).

## 4. Требования к API (публичные типы)

### 4.1. Типы идентификаторов
Добавить/использовать в core crate (если уже есть — адаптировать):

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum EntityId {
  Entid(i64),
  LookupRef { attr: &'static str, value: Value },
  Temp(i64), // или String
}
```

### 4.2. TxOp
```rust
pub enum TxOp {
  Assert { e: EntityId, a: &'static str, v: Value },
  Retract { e: EntityId, a: &'static str, v: Value },
  RetractAttr { e: EntityId, a: &'static str },
  // Optional:
  Ensure { pred: EnsurePred },
}
```

### 4.3. Patch типы
```rust
pub enum Patch<T> { NoChange, Set(T), Unset }

pub struct ManyPatch<T> {
  pub add: Vec<T>,
  pub remove: Vec<T>,
  pub clear: bool,
}
```

### 4.4. View metadata
Сгенерировать трейты (в core crate, реализуемые макросом):

```rust
pub enum FieldKind {
  Scalar,
  Ref { nested: ViewTypeId },
  Backref { nested: ViewTypeId },
}

pub struct FieldSpec {
  pub rust_name: &'static str,  // "car"
  pub attr: &'static str,       // ":person/car" или ":car/owner" для backref (forward ident)
  pub kind: FieldKind,
  pub cardinality_many: bool,
}

pub trait EntityViewSpec {
  const NS: &'static str;
  const FIELDS: &'static [FieldSpec];
}
```

`ViewTypeId` может быть `TypeId` или `'static str` с именем типа, в MVP допустимо `'static str`.

## 5. Правила маппинга полей → атрибутов

1) **Namespace по умолчанию**:
   - если есть `#[entity(ns="...")]` → использовать его
   - иначе `ns = snake_case(struct_name)`
2) **Имя атрибута по умолчанию для поля**:
   - `":{ns}/{snake_case(field_name)}"`
3) **Override атрибута**:
   - `#[attr(":custom/ident")]` задаёт точное значение
4) `#[attr(":db/id")]` допустим для `id` в view.
5) `#[entity_id]` для patch: поле id должно иметь тип `EntityId` (или `i64`, если принято в проекте; предпочтительно `EntityId`).

## 6. Refs и Backrefs

### 6.1. Forward ref
Сценарий: `[:person/car person-e car-e]`.

```rust
#[ref(attr=":person/car")]
pub car: Option<CarView>;
```

Требования:
- `FieldSpec.kind = Ref{nested=CarView}`
- cardinality определяется типом (`Option`/`Vec`)
- `attr` — фактический ident `:person/car`

### 6.2. Reverse ref (backref)
Сценарий: `[:car/owner car-e person-e]`, но в `PersonView` хотим `car`.

```rust
#[backref(attr=":car/owner")]
pub car: Option<CarView>;
```

Требования:
- `FieldSpec.kind = Backref{nested=CarView}`
- `attr` хранит forward ident `:car/owner` (репозиторий сам знает, что в pull это `:_car/owner`)
- cardinality: `Option` → one, `Vec` → many
- В случае `Option`, если найдено >1 значений, поведение MVP: **ошибка** `BackrefCardinalityViolation` (не молча брать первый).

## 7. Генерация Patch → TxOps

### 7.1. Cardinality-one (Patch<T>)
Поле типа `Patch<T>` (где `T` scalar или `EntityId`):
- `NoChange` → ничего
- `Set(x)` → `TxOp::Assert { e, a, v }`
- `Unset` → `TxOp::RetractAttr { e, a }`

### 7.2. Cardinality-many (ManyPatch<T>)
- если `clear=true` → `RetractAttr`
- `add` → `Assert` на каждый элемент
- `remove` → `Retract` на каждый элемент

### 7.3. Преобразование значений в `Value`
- Требование: любой `T` использующийся в Patch должен уметь `Into<Value>` или `TryInto<Value>`.
- Для enums/keywords предоставить helper trait `KeywordEnum` или ручной impl в тестах.

## 8. Pull pattern (форма данных)

MVP: макрос генерит **metadata**, а конкретное построение pull‑pattern делает репозиторий.  
Однако для удобства разрешается в MVP добавить метод‑хелпер:

```rust
pub trait EntityViewPull {
  fn pull_pattern(depth: usize) -> PullPattern; // depth=0 -> scalars only; depth>=1 -> refs/backrefs
}
```

Правила:
- Scalars всегда включаются
- Ref/backref включаются если `depth > 0`
- nested вызывается с `depth-1`
- Не должно быть бесконечной рекурсии (циклы). Минимум: дедуп по типу или по attr+type в стеке.

## 9. Ошибки и сообщения

MVP ошибки compile-time:
- неизвестный параметр в `#[entity(...)]`, `#[ref(...)]`, `#[backref(...)]`
- отсутствие `#[entity_id]` в `EntityPatch`
- `#[entity_id]` задан на поле неподходящего типа
- `#[backref]` на scalar поле
- конфликт `#[attr]` и `#[ref/backref]` (если одновременно)

Runtime ошибки (репозиторий, если реализуется в примере):
- `BackrefCardinalityViolation`
- `MissingEntity` (если pull/ref не найден, а поле non-optional)
- `TypeMismatch` (Value не конвертируется)

## 10. Примеры (должны быть в репозитории)

### 10.1. Person + Car (backref)
```rust
#[derive(EntityView)]
#[entity(ns="person")]
pub struct PersonView {
  #[attr(":db/id")]
  pub id: i64,

  pub name: String,

  #[backref(attr=":car/owner")]
  pub car: Option<CarView>,
}

#[derive(EntityView)]
#[entity(ns="car")]
pub struct CarView {
  #[attr(":db/id")]
  pub id: i64,

  pub model: String,

  #[ref(attr=":car/owner")]
  pub owner: EntityId,
}
```

### 10.2. OrderPatch
```rust
#[derive(EntityPatch)]
#[entity(ns="order")]
pub struct OrderPatch {
  #[entity_id]
  pub id: EntityId,

  pub status: Patch<OrderStatus>,

  #[attr(":order/customer")]
  pub customer: Patch<EntityId>,

  pub tags: ManyPatch<String>,
}
```

И ожидаемый `to_tx()` для:
- `status=Set(Paid)` → `Assert(:order/status, :status/paid)`
- `tags.add=["vip"]` → `Assert(:order/tags,"vip")`

## 11. Структура изменений в кодовой базе (план работ)

1) Создать crate `entity_codegen` (proc-macro)  
   - `syn`/`quote`/`proc-macro2`
2) В core crate (или рядом) добавить/экспортировать:
   - `EntityId`, `TxOp`, `Patch`, `ManyPatch`, `FieldSpec`, `EntityViewSpec`
3) Реализовать `derive(EntityView)`
   - парсинг атрибутов
   - вычисление namespace default
   - вычисление attr default
   - определение FieldKind по аннотациям и типам
   - генерация `impl EntityViewSpec for T`
4) Реализовать `derive(EntityPatch)`
   - поиск `#[entity_id]`
   - генерация `to_tx()`
   - поддержка `#[attr]` overrides
5) Добавить tests (compile tests + runtime tests)
   - compile-fail тесты (желательно `trybuild`)
   - unit tests для `FIELDS` и `to_tx()`
6) Добавить docs/README с examples и оговорками.

## 12. Acceptance Criteria (готово, когда)

- В проекте есть derive‑макросы `EntityView` и `EntityPatch`.
- Примеры `PersonView`/`CarView`/`OrderPatch` компилируются и тесты проходят.
- Namespace default и `#[entity(ns=...)]` работают.
- `#[attr]` override работает.
- `#[ref]` и `#[backref]` порождают корректные `FieldSpec`.
- `EntityPatch::to_tx()` генерит корректный набор `TxOp` согласно правилам.
- Есть документация и минимум 3 кейса в тестах:
  1) scalar + default attr
  2) forward ref
  3) backref (one) + ошибка при >1 значении (на уровне репозитория/хелпера).

## 13. Ограничения MVP

- Не требуется реализовывать полноценный transactor или pull engine — только metadata + tx ops generation.
- Не требуется автоматический diff для mutable structs (используем Patch).
- Не требуется Graph depth policy кроме простого `depth` (если делается хелпер).

---

## Примечания для Copilot (implementation hints)

- Для `snake_case` использовать собственную функцию (без внешних зависимостей) или `convert_case` (если допустимо).
- Для определения `Option<T>`/`Vec<T>` парсить `syn::Type::Path` и смотреть идентификатор последнего сегмента (`Option`, `Vec`).
- Для `nested` типа брать `T` из `Option<T>`/`Vec<T>`.
- Хранить `nested` в metadata как строку `stringify!(Type)` в MVP.
- `to_tx()` генерить через match по `Patch`/`ManyPatch`, избегая аллокаций где можно.
