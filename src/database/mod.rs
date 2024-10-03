/*
 *  This Source Code Form is subject to the terms of the Mozilla Public
 *  License, v. 2.0. If a copy of the MPL was not distributed with this
 *  file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use sqlx::{postgres::PgConnectOptions, Database, Executor, PgPool, Row};

use crate::errors::Error;

pub mod entities;

pub trait Queryer<'c, DB: Database>: Executor<'c, Database = DB> {}

impl<'c> Queryer<'c, sqlx::Postgres> for &PgPool {}

static DEFAULT_CONNECTION_HOST: &str = "localhost";
static DEFAULT_CONNECTION_USERNAME: &str = "symfonia";
static DEFAULT_CONNECTION_PASSWORD: &str = "symfonia";
static DEFAULT_CONNECTION_NAME: &str = "symfonia";
static DEFAULT_CONNECTION_PORT: u16 = 5432;

pub async fn establish_connection() -> Result<sqlx::PgPool, Error> {
    let db_url = std::env::var("DATABASE_HOST").unwrap_or_else(|_| {
        log::warn!(target: "symfonia::db", "You did not specify `DATABASE_HOST` environment variable, defaulting to '{DEFAULT_CONNECTION_HOST}'.");
        DEFAULT_CONNECTION_HOST.to_string()
    });
    let connect_options = PgConnectOptions::new()
        .host(&db_url)
        .port(std::env::var("DATABASE_PORT").unwrap_or_else(|_| {
            log::warn!(target: "symfonia::db", "You did not specify `DATABASE_PORT` environment variable. Defaulting to '{DEFAULT_CONNECTION_PORT}'.");
            DEFAULT_CONNECTION_PORT.to_string()
        }).parse::<u16>().expect("DATABASE_PORT must be a valid 16 bit unsigned integer."))
        .username(&std::env::var("DATABASE_USERNAME").unwrap_or_else(|_| {
            log::warn!(target: "symfonia::db", "You did not specify `DATABASE_USERNAME` environment variable. Defaulting to '{DEFAULT_CONNECTION_USERNAME}'.");
            DEFAULT_CONNECTION_USERNAME.to_string()
        }))
        .password(&std::env::var("DATABASE_PASSWORD").unwrap_or_else(|_| {
            log::warn!(target: "symfonia::db", "You did not specify `DATABASE_PASSWORD` environment variable. Defaulting to '{DEFAULT_CONNECTION_PASSWORD}'.");
            DEFAULT_CONNECTION_PASSWORD.to_string()
        }))
        .database(&std::env::var("DATABASE_NAME").unwrap_or_else(|_| {
            log::warn!(target: "symfonia::db", "You did not specify `DATABASE_NAME` environment variable. Defaulting to '{DEFAULT_CONNECTION_NAME}'.");
            DEFAULT_CONNECTION_NAME.to_string()
        }));
    let pool = PgPool::connect_with(connect_options).await?;
    //install_default_drivers();

    Ok(pool)
}

pub async fn check_migrating_from_spacebar(db: &PgPool) -> Result<bool, Error> {
    let res = sqlx::query("SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = current_database() AND table_name = 'migrations')")
        .fetch_one(db)
        .await?;

    Ok(res.get(0))
}

pub async fn check_fresh_db(db: &PgPool) -> Result<bool, Error> {
    let res = sqlx::query("SELECT COUNT(*) FROM config")
        .fetch_one(db)
        .await?;
    let c: i64 = res.get(0);

    Ok(c == 0)
}

pub async fn delete_spacebar_migrations(db: &PgPool) -> Result<(), Error> {
    sqlx::query("DROP TABLE migrations").execute(db).await?;

    Ok(())
}

pub async fn seed_config(db: &PgPool) -> Result<(), Error> {
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('api_activeVersions_0', '"6"');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('api_activeVersions_1', '"7"');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('api_activeVersions_2', '"8"');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('api_activeVersions_3', '"9"');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('api_defaultVersion', '"9"');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('api_endpointPublic', null);"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('cdn_endpointClient', null);"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('cdn_endpointPrivate', null);"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('cdn_endpointPublic', null);"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('cdn_imagorServerUrl', null);"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('cdn_resizeHeightMax', '1000');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('cdn_resizeWidthMax', '1000');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('defaults_guild_afkTimeout', '300');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('defaults_guild_defaultMessageNotifications', '1');"#).execute(db).await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('defaults_guild_explicitContentFilter', '0');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('defaults_guild_maxPresences', '250000');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('defaults_guild_maxVideoChannelUsers', '200');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('defaults_user_premium', 'true');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('defaults_user_premiumType', '2');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('defaults_user_verified', 'true');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('email_mailgun_apiKey', null);"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('email_mailgun_domain', null);"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('email_mailjet_apiKey', null);"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('email_mailjet_apiSecret', null);"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('email_provider', null);"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('email_sendgrid_apiKey', null);"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('email_smtp_host', null);"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('email_smtp_password', null);"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('email_smtp_port', null);"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('email_smtp_secure', 'false');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('email_smtp_username', null);"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('external_twitter', null);"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('gateway_endpointClient', null);"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('gateway_endpointPrivate', null);"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('gateway_endpointPublic', null);"#)
        .execute(db)
        .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('general_autoCreateBotUsers', 'false');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('general_correspondenceEmail', null);"#)
        .execute(db)
        .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('general_correspondenceUserID', null);"#,
    )
    .execute(db)
    .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('general_frontPage', null);"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('general_image', null);"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('general_instanceDescription', '"This is a Spacebar instance made in the pre-release days"');"#).execute(db).await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('general_instanceId', '"1160033703750033437"');"#).execute(db).await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('general_instanceName', '"Spacebar Instance"');"#).execute(db).await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('general_tosPage', null);"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('gif_apiKey', '"LIVDSRZULELA"');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('gif_enabled', 'true');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('gif_provider', '"tenor"');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('guild_autoJoin_canLeave', 'true');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('guild_autoJoin_enabled', 'true');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('guild_discovery_limit', '24');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('guild_discovery_offset', '0');"#)
        .execute(db)
        .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('guild_discovery_showAllGuilds', 'false');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('guild_discovery_useRecommendation', 'false');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('kafka_brokers', null);"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('limits_absoluteRate_register_enabled', 'true');"#).execute(db).await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('limits_absoluteRate_register_limit', '25');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('limits_absoluteRate_register_window', '3600000');"#).execute(db).await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('limits_absoluteRate_sendMessage_enabled', 'true');"#).execute(db).await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('limits_absoluteRate_sendMessage_limit', '200');"#).execute(db).await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('limits_absoluteRate_sendMessage_window', '60000');"#).execute(db).await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('limits_channel_maxPins', '500');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('limits_channel_maxTopic', '1024');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('limits_channel_maxWebhooks', '100');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('limits_guild_maxChannels', '65535');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('limits_guild_maxChannelsInCategory', '65535');"#).execute(db).await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('limits_guild_maxEmojis', '2000');"#)
        .execute(db)
        .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('limits_guild_maxMembers', '25000000');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('limits_guild_maxRoles', '1000');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('limits_message_maxAttachmentSize', '1073741824');"#).execute(db).await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('limits_message_maxBulkDelete', '1000');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('limits_message_maxCharacters', '1048576');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('limits_message_maxEmbedDownloadSize', '5242880');"#).execute(db).await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('limits_message_maxReactions', '2048');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('limits_message_maxTTSCharacters', '160');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('limits_rate_enabled', 'false');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('limits_rate_error_count', '10');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('limits_rate_error_window', '5');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('limits_rate_global_count', '250');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('limits_rate_global_window', '5');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('limits_rate_ip_count', '500');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('limits_rate_ip_window', '5');"#)
        .execute(db)
        .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('limits_rate_routes_auth_login_count', '5');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('limits_rate_routes_auth_login_window', '60');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('limits_rate_routes_auth_register_count', '2');"#).execute(db).await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('limits_rate_routes_auth_register_window', '43200');"#).execute(db).await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('limits_rate_routes_channel_count', '10');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('limits_rate_routes_channel_window', '5');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('limits_rate_routes_guild_count', '5');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('limits_rate_routes_guild_window', '5');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('limits_rate_routes_webhook_count', '10');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('limits_rate_routes_webhook_window', '5');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('limits_user_maxFriends', '5000');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('limits_user_maxGuilds', '1048576');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('limits_user_maxUsername', '32');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('login_requireCaptcha', 'false');"#)
        .execute(db)
        .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('login_requireVerification', 'false');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('metrics_timeout', '30000');"#)
        .execute(db)
        .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('passwordReset_requireCaptcha', 'false');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('rabbitmq_host', null);"#)
        .execute(db)
        .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('regions_available_0_custom', 'false');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('regions_available_0_deprecated', 'false');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('regions_available_0_endpoint', '"127.0.0.1:3004"');"#).execute(db).await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('regions_available_0_id', '"spacebar"');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('regions_available_0_name', '"spacebar"');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('regions_available_0_vip', 'false');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('regions_default', '"spacebar"');"#)
        .execute(db)
        .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('regions_useDefaultAsOptimal', 'true');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('register_allowMultipleAccounts', 'true');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('register_allowNewRegistration', 'true');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('register_blockProxies', 'true');"#)
        .execute(db)
        .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('register_dateOfBirth_minimum', '13');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('register_dateOfBirth_required', 'true');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('register_defaultRights', '"875069521787904"');"#).execute(db).await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('register_disabled', 'false');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('register_email_allowlist', 'false');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('register_email_blocklist', 'true');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('register_email_required', 'false');"#)
        .execute(db)
        .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('register_guestsRequireInvite', 'true');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('register_incrementingDiscriminators', 'false');"#).execute(db).await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('register_password_minLength', '8');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('register_password_minNumbers', '2');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('register_password_minSymbols', '0');"#)
        .execute(db)
        .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('register_password_minUpperCase', '2');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('register_password_required', 'false');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('register_requireCaptcha', 'true');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('register_requireInvite', 'false');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('security_autoUpdate', 'true');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('security_captcha_enabled', 'false');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('security_captcha_secret', null);"#)
        .execute(db)
        .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('security_captcha_service', '"hcaptcha"');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('security_captcha_sitekey', null);"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('security_defaultRegistrationTokenExpiration', '604800000');"#).execute(db).await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('security_forwardedFor', null);"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('security_ipdataApiKey', '"eca677b284b3bac29eb72f5e496aa9047f26543605efe99ff2ce35c9"');"#).execute(db).await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('security_jwtSecret', '"7CYWGvQ8pB9f6B3EkTvbYvZUhA3AwXc8cAsg8KFvHTcH7pN7hK0DSk1w+h+A1bI3WYmdV2UqFWBvFajT3IZ3EqzI8h4Ttq/iNB5PsSC1YPpg4geG58fOFF691Me3aE7MmUAFxDpAfUAMNjG7sX6V3YTEaIWYTfGiDZPr2X6P7QRNlo8sEJ833kXXMCGz0/Zs5uZGlbm7/qFeNNRBCS3QW9hALZqf72rB0rc2zEB7S/I48WW1UTwQOLhqAc/neHz/PTIPYHPxnDvWfReaSzWGVuOXYrP/2YR6DKCwbulzL8/Lt4LdJcSf9ZyVfGoOm4uZ5Ynk3xElYFfuNgCTs0Lujg=="');"#).execute(db).await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('security_mfaBackupCodeCount', '10');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('security_requestSignature', '"q5AyKYvr6oILJeQJbeEKjAInwahdGhZmr+FGO8RBRuo="');"#).execute(db).await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('security_statsWorldReadable', 'true');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('security_twoFactor_generateBackupCodes', 'true');"#).execute(db).await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('sentry_enabled', 'false');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('sentry_endpoint', '"https://05e8e3d005f34b7d97e920ae5870a5e5@sentry.thearcanebrony.net/6"');"#).execute(db).await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('sentry_environment', '"quartz"');"#)
        .execute(db)
        .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('sentry_traceSampleRate', '1');"#)
        .execute(db)
        .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('templates_allowDiscordTemplates', 'true');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('templates_allowRaws', 'true');"#)
        .execute(db)
        .await?;
    sqlx::query(
        r#"INSERT INTO config (key, value) VALUES ('templates_allowTemplateCreation', 'true');"#,
    )
    .execute(db)
    .await?;
    sqlx::query(r#"INSERT INTO config (key, value) VALUES ('templates_enabled', 'true');"#)
        .execute(db)
        .await?;
    Ok(())
}
