//! Integration tests for Turbine
//!
//! These tests require a running Redis instance.
//! Run with: cargo test -p turbine-tests -- --ignored
//! Or use docker-compose to start Redis first.

/// Test that Redis broker can connect
#[tokio::test]
#[ignore = "requires Redis"]
async fn test_redis_broker_connection() {
    use turbine_broker::{Broker, RedisBroker};
    use turbine_broker::redis::RedisBrokerConfig;

    let config = RedisBrokerConfig {
        url: "redis://localhost:6379".to_string(),
        ..Default::default()
    };

    let broker = RedisBroker::with_config(config)
        .await
        .expect("Failed to connect to Redis");

    assert!(broker.is_connected().await, "Broker should be connected");
    println!("Redis broker connection test passed!");
}

/// Test that Redis backend can connect and store/retrieve results
#[tokio::test]
#[ignore = "requires Redis"]
async fn test_redis_backend_integration() {
    use turbine_backend::{Backend, RedisBackend};
    use turbine_backend::redis::RedisBackendConfig;
    use turbine_core::{TaskResult, TaskState, TaskId};

    let config = RedisBackendConfig {
        url: "redis://localhost:6379".to_string(),
        ..Default::default()
    };

    let backend = RedisBackend::with_config(config)
        .await
        .expect("Failed to connect to Redis");

    assert!(backend.is_connected().await, "Backend should be connected");

    let task_id = TaskId::new();

    // Create a task result
    let result = TaskResult {
        task_id: task_id.clone(),
        state: TaskState::Success,
        result: Some(serde_json::json!({
            "output": "test result",
            "count": 42
        })),
        error: None,
        traceback: None,
        created_at: chrono::Utc::now(),
    };

    // Store the result
    backend.store_result(&result, None)
        .await
        .expect("Failed to store result");

    // Retrieve the result
    let stored = backend.get_result(&task_id)
        .await
        .expect("Failed to get result");

    assert!(stored.is_some(), "Should find stored result");

    let retrieved = stored.unwrap();
    assert_eq!(retrieved.state, TaskState::Success);
    assert_eq!(retrieved.result, result.result);

    // Clean up
    backend.delete_result(&task_id)
        .await
        .expect("Failed to delete result");

    println!("Redis backend integration test passed!");
}

/// Test rate limiter functionality
#[tokio::test]
async fn test_rate_limiter() {
    use turbine_server::ratelimit::{RateLimiter, RateLimitConfigBuilder, RateLimitResult};

    let config = RateLimitConfigBuilder::new()
        .default_rate(10.0, 3)  // 10 per second, burst of 3
        .build();

    let limiter = RateLimiter::new(config);

    // First 3 should be allowed (burst)
    for i in 0..3 {
        let result = limiter.check("test_task", "default").await;
        assert!(
            matches!(result, RateLimitResult::Allowed),
            "Request {} should be allowed", i
        );
    }

    // 4th should be limited
    let result = limiter.check("test_task", "default").await;
    assert!(
        matches!(result, RateLimitResult::Limited { .. }),
        "Request 4 should be rate limited"
    );

    println!("Rate limiter test passed!");
}

/// Test task routing
#[tokio::test]
async fn test_task_router() {
    use turbine_server::router::{
        TaskRouter, RouterConfigBuilder, RoutingRule,
        MatchCondition, RoutingDecision
    };
    use turbine_core::{Task, Message, Serializer};

    let config = RouterConfigBuilder::new()
        .default_queue("default")
        .priority_queues(false, 3)
        .rule(
            RoutingRule::new("email_router")
                .with_condition(MatchCondition::TaskNamePrefix {
                    prefix: "email_".to_string()
                })
                .route_to("emails")
        )
        .rule(
            RoutingRule::new("high_priority")
                .with_condition(MatchCondition::PriorityRange { min: 200, max: 255 })
                .route_to("urgent")
        )
        .build();

    let router = TaskRouter::new(config);

    // Test email routing
    let email_task = Task::new("email_send_welcome");
    let email_msg = Message::from_task(email_task.clone(), Serializer::Json).unwrap();

    let decision = router.route(&email_task, &email_msg).await;
    assert!(matches!(decision, RoutingDecision::SingleQueue(ref q) if q == "emails"));

    // Test default routing
    let other_task = Task::new("process_data");
    let other_msg = Message::from_task(other_task.clone(), Serializer::Json).unwrap();

    let decision = router.route(&other_task, &other_msg).await;
    assert!(matches!(decision, RoutingDecision::SingleQueue(ref q) if q == "default"));

    println!("Task router test passed!");
}

/// Test multi-tenancy
#[tokio::test]
async fn test_multi_tenancy() {
    use turbine_server::multitenancy::{
        TenantManager, MultiTenancyConfig, Tenant, TenantId
    };

    let config = MultiTenancyConfig {
        enabled: true,
        queue_namespace_pattern: "{tenant}:{queue}".to_string(),
        ..Default::default()
    };

    let manager = TenantManager::new(config);

    // Create a tenant
    let mut tenant = Tenant::new("acme", "Acme Corporation");
    let api_key = tenant.generate_api_key();

    manager.create_tenant(tenant.clone())
        .await
        .expect("Failed to create tenant");

    // Retrieve by ID
    let retrieved = manager.get_tenant(&TenantId::new("acme"))
        .await
        .expect("Should find tenant");
    assert_eq!(retrieved.name, "Acme Corporation");

    // Retrieve by API key
    let by_key = manager.get_tenant_by_api_key(&api_key)
        .await
        .expect("Should find tenant by API key");
    assert_eq!(by_key.id.as_str(), "acme");

    // Test queue namespacing
    let namespaced = manager.namespace_queue(&TenantId::new("acme"), "tasks");
    assert_eq!(namespaced, "acme:tasks");

    // List tenants
    let all = manager.list_tenants().await;
    assert_eq!(all.len(), 1);

    // Delete tenant
    manager.delete_tenant(&TenantId::new("acme"))
        .await
        .expect("Failed to delete tenant");

    let deleted = manager.get_tenant(&TenantId::new("acme")).await;
    assert!(deleted.is_none());

    println!("Multi-tenancy test passed!");
}

/// Test encryption
#[test]
fn test_encryption() {
    use turbine_core::{EncryptionKey, Encryptor, EncryptionConfig, KeyManager};

    // Generate key
    let key = EncryptionKey::generate();

    let config = EncryptionConfig {
        enabled: true,
        key_id: "test-key-v1".to_string(),
        encrypt_args: true,
        encrypt_results: true,
        encrypt_metadata: false,
    };

    let encryptor = Encryptor::new(&key, config.clone());

    // Test encrypt/decrypt
    let plaintext = b"sensitive task arguments";
    let encrypted = encryptor.encrypt(plaintext).expect("Encryption failed");
    let decrypted = encryptor.decrypt(&encrypted).expect("Decryption failed");

    assert_eq!(plaintext.as_slice(), decrypted.as_slice());

    // Test key rotation
    let key2 = EncryptionKey::generate();
    let mut manager = KeyManager::new("key-v1", key);

    // Encrypt with v1
    let enc1 = manager.encryptor(config.clone()).encrypt(b"data v1").unwrap();

    // Rotate to v2
    manager.rotate_key("key-v2", key2);

    // Can still decrypt v1 data
    let dec1 = manager.decrypt(&enc1, &config).expect("Should decrypt old data");
    assert_eq!(dec1, b"data v1");

    println!("Encryption test passed!");
}

/// Test TLS configuration
#[test]
fn test_tls_config() {
    use turbine_core::{TlsConfig, RedisTlsConfig, AmqpTlsConfig};

    // Test disabled config
    let disabled = TlsConfig::disabled();
    assert!(!disabled.enabled);
    assert!(disabled.validate().is_ok());

    // Test Redis URL conversion
    let redis_tls = RedisTlsConfig {
        tls: TlsConfig { enabled: true, ..Default::default() },
        starttls: false,
    };
    assert_eq!(
        redis_tls.apply_to_url("redis://localhost:6379"),
        "rediss://localhost:6379"
    );

    // Test AMQP URL conversion
    let amqp_tls = AmqpTlsConfig {
        tls: TlsConfig { enabled: true, ..Default::default() },
        external_auth: false,
    };
    assert_eq!(
        amqp_tls.apply_to_url("amqp://localhost:5672"),
        "amqps://localhost:5672"
    );

    println!("TLS config test passed!");
}

/// Test core task and message types
#[test]
fn test_core_types() {
    use turbine_core::{Task, Message, Serializer, TaskState};

    // Test task creation
    let task = Task::new("my_task");
    assert_eq!(task.name, "my_task");
    assert_eq!(task.meta.state, TaskState::Pending);

    // Test message creation
    let message = Message::from_task(task, Serializer::Json)
        .expect("Failed to create message");
    assert_eq!(message.task.as_ref().unwrap().name, "my_task");

    println!("Core types test passed!");
}

/// Test schedule entry creation
#[test]
fn test_schedule_entry() {
    use turbine_server::scheduler::ScheduleEntry;

    // Test cron schedule creation
    let entry = ScheduleEntry::cron("hourly_check", "check_status", "0 * * * *");

    assert_eq!(entry.id, "hourly_check");
    assert_eq!(entry.task_name, "check_status");
    assert!(entry.enabled);

    // Test interval schedule
    let interval_entry = ScheduleEntry::interval("every_5min", "heartbeat", 300);
    assert_eq!(interval_entry.id, "every_5min");

    println!("Schedule entry test passed!");
}
