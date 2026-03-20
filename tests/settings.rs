use std::collections::BTreeMap;

use aiwattcoach::Settings;

#[test]
fn settings_load_required_values_from_map() {
    let settings = Settings::from_map(&BTreeMap::from([
        ("APP_NAME".to_string(), "AiWattCoach".to_string()),
        ("SERVER_HOST".to_string(), "127.0.0.1".to_string()),
        ("SERVER_PORT".to_string(), "3000".to_string()),
        (
            "MONGODB_URI".to_string(),
            "mongodb://localhost:27017".to_string(),
        ),
        ("MONGODB_DATABASE".to_string(), "aiwattcoach".to_string()),
    ]))
    .unwrap();

    assert_eq!(settings.app_name, "AiWattCoach");
    assert_eq!(settings.server.host, "127.0.0.1");
    assert_eq!(settings.server.port, 3000);
    assert_eq!(settings.mongo.uri, "mongodb://localhost:27017");
    assert_eq!(settings.mongo.database, "aiwattcoach");
}
