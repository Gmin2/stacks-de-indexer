//! Dynamic GraphQL schema generation from indexer config.
//!
//! For each configured event table, this module generates:
//! - A GraphQL object type with system columns + `data` field.
//! - A root query field with `limit`, `offset`, `orderBy`, `orderDir`, `where` args.
//! - A subscription field that streams new events via WebSocket.

use async_graphql::dynamic::*;
use std::sync::Arc;
use tokio::sync::broadcast;

use stacks_indexer_core::config::IndexerConfig;
use stacks_indexer_storage::Database;

/// Build the complete GraphQL schema from config.
///
/// The schema is dynamic — types and fields are generated at runtime based on
/// which tables are configured in the YAML file.
pub fn build_schema(
    config: &IndexerConfig,
    db: Arc<Database>,
    event_tx: broadcast::Sender<serde_json::Value>,
) -> anyhow::Result<Schema> {
    let mut query_fields: Vec<Field> = Vec::new();
    let mut type_objects: Vec<Object> = Vec::new();
    let mut sub_fields: Vec<SubscriptionField> = Vec::new();

    for source in &config.sources {
        for event_cfg in &source.events {
            let table_name = event_cfg.table.clone();
            let type_name = to_pascal_case(&table_name);

            // Object type for this table
            let obj = Object::new(&type_name)
                .field(scalar_field("_id", TypeRef::INT))
                .field(scalar_field("_block_height", TypeRef::INT))
                .field(scalar_field("_block_hash", TypeRef::STRING))
                .field(scalar_field("_tx_id", TypeRef::STRING))
                .field(scalar_field("_event_index", TypeRef::INT))
                .field(scalar_field("_timestamp", TypeRef::INT))
                .field(scalar_field("_event_type", TypeRef::STRING))
                .field(scalar_field("data", TypeRef::STRING));
            type_objects.push(obj);

            // Query field
            let db_clone = db.clone();
            let tbl = table_name.clone();
            let tn = type_name.clone();

            let qf = Field::new(&table_name, TypeRef::named_nn_list_nn(&tn), move |ctx| {
                let db = db_clone.clone();
                let table = tbl.clone();
                FieldFuture::new(async move {
                    let limit = arg_u64(&ctx, "limit").unwrap_or(100) as u32;
                    let offset = arg_u64(&ctx, "offset").unwrap_or(0) as u32;
                    let order_field = arg_string(&ctx, "orderBy");
                    let order_dir = arg_string(&ctx, "orderDir").unwrap_or("DESC".into());
                    let filters = parse_where(&ctx);

                    let order = order_field.as_deref().map(|f| (f, order_dir.as_str()));
                    let (rows, _) = db.query_table(&table, &filters, order, limit, offset)?;

                    let items: Vec<FieldValue> =
                        rows.into_iter().map(FieldValue::owned_any).collect();
                    Ok(Some(FieldValue::list(items)))
                })
            })
            .argument(InputValue::new("limit", TypeRef::named(TypeRef::INT)))
            .argument(InputValue::new("offset", TypeRef::named(TypeRef::INT)))
            .argument(InputValue::new("orderBy", TypeRef::named(TypeRef::STRING)))
            .argument(InputValue::new("orderDir", TypeRef::named(TypeRef::STRING)))
            .argument(InputValue::new("where", TypeRef::named(TypeRef::STRING)));
            query_fields.push(qf);

            // Subscription
            let tx_clone = event_tx.clone();
            let sub_table = table_name.clone();
            let sf = SubscriptionField::new(
                &format!("on_{table_name}"),
                TypeRef::named_nn(TypeRef::STRING),
                move |_| {
                    let mut rx = tx_clone.subscribe();
                    let t = sub_table.clone();
                    SubscriptionFieldFuture::new(async move {
                        let stream = async_stream::stream! {
                            while let Ok(ev) = rx.recv().await {
                                if ev.get("_table").and_then(|v| v.as_str()) == Some(&t) {
                                    yield Ok(FieldValue::value(ev.to_string()));
                                }
                            }
                        };
                        Ok(stream)
                    })
                },
            );
            sub_fields.push(sf);
        }
    }

    // Query root
    let mut query = Object::new("Query");
    query = query.field(Field::new("health", TypeRef::named_nn(TypeRef::STRING), |_| {
        FieldFuture::new(async { Ok(Some(FieldValue::value("ok"))) })
    }));

    let db_st = db.clone();
    query = query.field(Field::new(
        "indexerStatus",
        TypeRef::named_nn(TypeRef::STRING),
        move |_| {
            let db = db_st.clone();
            FieldFuture::new(async move {
                let (h, hash) = db.get_last_processed_block()?;
                Ok(Some(FieldValue::value(
                    serde_json::json!({"last_block_height": h, "last_block_hash": hash}).to_string(),
                )))
            })
        },
    ));
    for f in query_fields {
        query = query.field(f);
    }

    // Subscription root
    let mut subscription = Subscription::new("Subscription");
    let block_tx = event_tx.clone();
    subscription = subscription.field(SubscriptionField::new(
        "newBlock",
        TypeRef::named_nn(TypeRef::STRING),
        move |_| {
            let mut rx = block_tx.subscribe();
            SubscriptionFieldFuture::new(async move {
                let stream = async_stream::stream! {
                    while let Ok(ev) = rx.recv().await {
                        if ev.get("_type").and_then(|v| v.as_str()) == Some("new_block") {
                            yield Ok(FieldValue::value(ev.to_string()));
                        }
                    }
                };
                Ok(stream)
            })
        },
    ));
    for f in sub_fields {
        subscription = subscription.field(f);
    }

    // Assemble
    let mut builder = Schema::build(query.type_name(), None, Some(subscription.type_name()));
    builder = builder.register(query).register(subscription);
    for obj in type_objects {
        builder = builder.register(obj);
    }

    builder
        .finish()
        .map_err(|e| anyhow::anyhow!("failed to build GraphQL schema: {e}"))
}

// Helpers

/// Create a scalar field that extracts its value from the parent's `owned_any`
/// `serde_json::Value` object.
fn scalar_field(name: &str, type_ref: &str) -> Field {
    let field_name = name.to_string();
    Field::new(name, TypeRef::named_nn(type_ref), move |ctx| {
        let name = field_name.clone();
        FieldFuture::new(async move {
            let parent = match ctx.parent_value.try_downcast_ref::<serde_json::Value>() {
                Ok(v) => v,
                Err(_) => return Ok(None),
            };
            let val = parent.get(&name).cloned().unwrap_or(serde_json::Value::Null);
            match val {
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        Ok(Some(FieldValue::value(i)))
                    } else if let Some(f) = n.as_f64() {
                        Ok(Some(FieldValue::value(f)))
                    } else {
                        Ok(Some(FieldValue::value(n.to_string())))
                    }
                }
                serde_json::Value::String(s) => Ok(Some(FieldValue::value(s))),
                serde_json::Value::Bool(b) => Ok(Some(FieldValue::value(b))),
                serde_json::Value::Null => Ok(Some(FieldValue::value(""))),
                other => Ok(Some(FieldValue::value(other.to_string()))),
            }
        })
    })
}

fn arg_u64(ctx: &ResolverContext<'_>, name: &str) -> Option<u64> {
    ctx.args.try_get(name).ok().and_then(|v| v.u64().ok())
}

fn arg_string(ctx: &ResolverContext<'_>, name: &str) -> Option<String> {
    ctx.args
        .try_get(name)
        .ok()
        .and_then(|v| v.string().ok())
        .map(|s| s.to_string())
}

/// Parse the `where` argument: comma-separated `field:op:value` triples.
fn parse_where(ctx: &ResolverContext<'_>) -> Vec<(String, String, serde_json::Value)> {
    let Some(s) = arg_string(ctx, "where") else {
        return Vec::new();
    };
    s.split(',')
        .filter_map(|part| {
            let parts: Vec<&str> = part.splitn(3, ':').collect();
            if parts.len() == 3 {
                let val = serde_json::from_str(parts[2])
                    .unwrap_or(serde_json::Value::String(parts[2].to_string()));
                Some((parts[0].to_string(), parts[1].to_string(), val))
            } else {
                None
            }
        })
        .collect()
}

fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|part| {
            let mut c = part.chars();
            match c.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect()
}
