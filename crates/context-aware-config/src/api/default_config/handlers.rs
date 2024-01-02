use super::types::CreateReq;
use crate::{
    db::{models::DefaultConfig, schema::default_configs::dsl::default_configs},
    helpers::validate_jsonschema,
};
use actix_web::{
    get, put,
    web::{self, Data, Json},
    HttpResponse, Scope,
};
use chrono::Utc;
use dashboard_auth::types::User;
use diesel::RunQueryDsl;
use jsonschema::{Draft, JSONSchema};
use serde_json::Value;
use service_utils::{
    service::types::{AppState, DbConnection},
    types as app,
};

pub fn endpoints() -> Scope {
    Scope::new("").service(create).service(get)
}

#[put("/{key}")]
async fn create(
    state: Data<AppState>,
    key: web::Path<String>,
    request: web::Json<CreateReq>,
    user: User,
    db_conn: DbConnection,
) -> HttpResponse {
    let DbConnection(mut conn) = db_conn;
    let req = request.into_inner();
    let schema = Value::Object(req.schema);
    if let Err(e) = validate_jsonschema(&state.default_config_validation_schema, &schema)
    {
        return HttpResponse::BadRequest().body(e);
    };
    let schema_compile_result = JSONSchema::options()
        .with_draft(Draft::Draft7)
        .compile(&schema);
    let jschema = match schema_compile_result {
        Ok(jschema) => jschema,
        Err(e) => {
            log::info!("Failed to compile as a Draft-7 JSON schema: {e}");
            return HttpResponse::BadRequest().body("Bad json schema.");
        }
    };

    match jschema.validate(&req.value) {
        Ok(_) => (),
        Err(_) => {
            log::info!("Validation for value with given JSON schema failed.");
            return HttpResponse::BadRequest()
                .body("Validation with given schema failed.");
        }
    };

    let new_default_config = DefaultConfig {
        key: key.into_inner(),
        value: req.value,
        schema: schema,
        created_by: user.email,
        created_at: Utc::now(),
    };

    let upsert = diesel::insert_into(default_configs)
        .values(&new_default_config)
        .execute(&mut conn);

    match upsert {
        Ok(_) => {
            return HttpResponse::Created().body("DefaultConfig created successfully.")
        }
        Err(e) => {
            log::info!("DefaultConfig creation failed with error: {e}");
            return HttpResponse::InternalServerError()
                .body("Failed to create DefaultConfig");
        }
    }
}

#[get("")]
async fn get(db_conn: DbConnection) -> app::Result<Json<Vec<DefaultConfig>>> {
    let DbConnection(mut conn) = db_conn;

    let result: Vec<DefaultConfig> = default_configs.get_results(&mut conn)?;
    Ok(Json(result))
}
