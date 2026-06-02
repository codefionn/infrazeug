#[cfg(test)]
mod tests {
    use crate::publish::{machine_keygen, publish_slice, register_machine_pubkey, PublishOptions};
    use crate::serve::apply_sealed_slice;
    use crate::store::PlanStore;
    use infrazeug_core::id::{MachineId, NodeId};
    use infrazeug_core::node::{Node, NodeBody, Targets};
    use infrazeug_core::slice::SliceMode;
    use infrazeug_core::Infra;
    use infrazeug_secrets::{verifying_key_from_seed, FsBackend};
    use infrazeug_shell::ShellOp;
    use std::sync::Arc;
    use tempfile::tempdir;
    use uuid::Uuid;

    #[tokio::test]
    async fn publish_and_apply_pull_slice() {
        let dir = tempdir().unwrap();
        let store = PlanStore::new(Arc::new(FsBackend::new(dir.path())));
        let machine = Uuid::new_v4();
        let mid = MachineId(machine);
        let key_path = dir.path().join("machine.key");
        let pubkey = machine_keygen(machine, &key_path).unwrap();
        register_machine_pubkey(&store, machine, pubkey)
            .await
            .unwrap();

        let mut infra = Infra::new();
        infra.machines.push(infrazeug_core::Machine {
            id: mid,
            name: "pull-host".into(),
            kind: infrazeug_core::MachineKind::Local,
            vars: Default::default(),
            groups: vec![],
            tags: vec![],
            max_parallel_nodes: None,
            lifecycle: infrazeug_core::machine::Lifecycle::Persistent,
            like: None,
            lazy: false,
        });
        let n = NodeId(Uuid::new_v4());
        infra.nodes.push(Node {
            id: n,
            name: "echo".into(),
            description: None,
            body: NodeBody::Shell(ShellOp::Run {
                argv: vec!["sh".into(), "-c".into(), "echo pull-ok".into()],
                cwd: None,
                env: Vec::new(),
            }),
            targets: Targets::Machine(mid),
            deps: vec![],
            tags: vec![],
            policy: Default::default(),
        });

        let seed = [7u8; 32];
        publish_slice(
            &infra,
            &store,
            machine,
            PublishOptions {
                agent_digest: Some("sha256:test".into()),
                signing_seed: Some(seed),
                signer_id: "test".into(),
            },
        )
        .await
        .unwrap();

        let trusted = [verifying_key_from_seed(&seed)];
        let empty = Infra::new();
        apply_sealed_slice(&empty, &store, machine, &key_path, &trusted)
            .await
            .unwrap();
    }

    /// A signed slice is rejected when its signer is not in the trust set, and
    /// when the trust set is empty (fail-closed).
    #[tokio::test]
    async fn apply_rejects_untrusted_and_empty_trust() {
        let dir = tempdir().unwrap();
        let store = PlanStore::new(Arc::new(FsBackend::new(dir.path())));
        let machine = Uuid::new_v4();
        let mid = MachineId(machine);
        let key_path = dir.path().join("machine.key");
        let pubkey = machine_keygen(machine, &key_path).unwrap();
        register_machine_pubkey(&store, machine, pubkey)
            .await
            .unwrap();

        let mut infra = Infra::new();
        infra.machines.push(infrazeug_core::Machine {
            id: mid,
            name: "pull-host".into(),
            kind: infrazeug_core::MachineKind::Local,
            vars: Default::default(),
            groups: vec![],
            tags: vec![],
            max_parallel_nodes: None,
            lifecycle: infrazeug_core::machine::Lifecycle::Persistent,
            like: None,
            lazy: false,
        });
        let n = NodeId(Uuid::new_v4());
        infra.nodes.push(Node {
            id: n,
            name: "echo".into(),
            description: None,
            body: NodeBody::Shell(ShellOp::Run {
                argv: vec!["sh".into(), "-c".into(), "echo pull-ok".into()],
                cwd: None,
                env: Vec::new(),
            }),
            targets: Targets::Machine(mid),
            deps: vec![],
            tags: vec![],
            policy: Default::default(),
        });

        let seed = [7u8; 32];
        publish_slice(
            &infra,
            &store,
            machine,
            PublishOptions {
                agent_digest: Some("sha256:test".into()),
                signing_seed: Some(seed),
                signer_id: "test".into(),
            },
        )
        .await
        .unwrap();

        let empty = Infra::new();
        // Empty trust set -> fail-closed.
        assert!(apply_sealed_slice(&empty, &store, machine, &key_path, &[])
            .await
            .is_err());
        // Signed by `seed`, but we only trust a different key.
        let wrong = [verifying_key_from_seed(&[9u8; 32])];
        assert!(
            apply_sealed_slice(&empty, &store, machine, &key_path, &wrong)
                .await
                .is_err()
        );
    }

    /// An unsigned slice is rejected even if the trust set is non-empty.
    #[tokio::test]
    async fn apply_rejects_unsigned_slice() {
        let dir = tempdir().unwrap();
        let store = PlanStore::new(Arc::new(FsBackend::new(dir.path())));
        let machine = Uuid::new_v4();
        let mid = MachineId(machine);
        let key_path = dir.path().join("machine.key");
        let pubkey = machine_keygen(machine, &key_path).unwrap();
        register_machine_pubkey(&store, machine, pubkey)
            .await
            .unwrap();

        let mut infra = Infra::new();
        infra.machines.push(infrazeug_core::Machine {
            id: mid,
            name: "pull-host".into(),
            kind: infrazeug_core::MachineKind::Local,
            vars: Default::default(),
            groups: vec![],
            tags: vec![],
            max_parallel_nodes: None,
            lifecycle: infrazeug_core::machine::Lifecycle::Persistent,
            like: None,
            lazy: false,
        });
        let n = NodeId(Uuid::new_v4());
        infra.nodes.push(Node {
            id: n,
            name: "echo".into(),
            description: None,
            body: NodeBody::Shell(ShellOp::Run {
                argv: vec!["true".into()],
                cwd: None,
                env: Vec::new(),
            }),
            targets: Targets::Machine(mid),
            deps: vec![],
            tags: vec![],
            policy: Default::default(),
        });

        // No signing seed -> slice carries no signatures.
        publish_slice(
            &infra,
            &store,
            machine,
            PublishOptions {
                agent_digest: None,
                signing_seed: None,
                signer_id: "test".into(),
            },
        )
        .await
        .unwrap();

        let trusted = [verifying_key_from_seed(&[7u8; 32])];
        let empty = Infra::new();
        assert!(
            apply_sealed_slice(&empty, &store, machine, &key_path, &trusted)
                .await
                .is_err()
        );
    }

    /// Tampering with the slice contents while keeping the original digest and a
    /// valid trusted signature is rejected (signed digest is bound to contents).
    #[tokio::test]
    async fn apply_rejects_tampered_contents() {
        use infrazeug_core::slice::PlanSlice;
        use infrazeug_secrets::{seal_bytes, unseal_bytes, MachineKeyPair};

        let dir = tempdir().unwrap();
        let store = PlanStore::new(Arc::new(FsBackend::new(dir.path())));
        let machine = Uuid::new_v4();
        let mid = MachineId(machine);
        let key_path = dir.path().join("machine.key");
        let pubkey = machine_keygen(machine, &key_path).unwrap();
        register_machine_pubkey(&store, machine, pubkey)
            .await
            .unwrap();

        let mut infra = Infra::new();
        infra.machines.push(infrazeug_core::Machine {
            id: mid,
            name: "pull-host".into(),
            kind: infrazeug_core::MachineKind::Local,
            vars: Default::default(),
            groups: vec![],
            tags: vec![],
            max_parallel_nodes: None,
            lifecycle: infrazeug_core::machine::Lifecycle::Persistent,
            like: None,
            lazy: false,
        });
        let n = NodeId(Uuid::new_v4());
        infra.nodes.push(Node {
            id: n,
            name: "echo".into(),
            description: None,
            body: NodeBody::Shell(ShellOp::Run {
                argv: vec!["sh".into(), "-c".into(), "echo pull-ok".into()],
                cwd: None,
                env: Vec::new(),
            }),
            targets: Targets::Machine(mid),
            deps: vec![],
            tags: vec![],
            policy: Default::default(),
        });

        let seed = [7u8; 32];
        publish_slice(
            &infra,
            &store,
            machine,
            PublishOptions {
                agent_digest: Some("sha256:test".into()),
                signing_seed: Some(seed),
                signer_id: "test".into(),
            },
        )
        .await
        .unwrap();

        // Attacker with the (public) machine key: unseal, mutate the embedded
        // node, keep the original digest + signature, re-seal, and re-publish.
        let pair = MachineKeyPair::read_private_file(&key_path).unwrap();
        let sealed = store.get_sealed_plan(machine).await.unwrap().unwrap();
        let plain = unseal_bytes(&sealed, pair.secret_bytes()).unwrap();
        let mut slice = PlanSlice::from_cbor(&plain).unwrap();
        slice.embedded_nodes[0].body = NodeBody::Shell(ShellOp::Run {
            argv: vec!["sh".into(), "-c".into(), "echo PWNED".into()],
            cwd: None,
            env: Vec::new(),
        });
        let tampered = seal_bytes(&slice.to_cbor().unwrap(), &pair.public).unwrap();
        store.put_sealed_plan(machine, &tampered).await.unwrap();

        let trusted = [verifying_key_from_seed(&seed)];
        let empty = Infra::new();
        assert!(
            apply_sealed_slice(&empty, &store, machine, &key_path, &trusted)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn pull_slice_rejects_wait() {
        let a = MachineId(Uuid::new_v4());
        let b = MachineId(Uuid::new_v4());
        let mut infra = Infra::new();
        for (id, name) in [(a, "a"), (b, "b")] {
            infra.machines.push(infrazeug_core::Machine {
                id,
                name: name.into(),
                kind: infrazeug_core::MachineKind::Local,
                vars: Default::default(),
                groups: vec![],
                tags: vec![],
                max_parallel_nodes: None,
                lifecycle: infrazeug_core::machine::Lifecycle::Persistent,
                like: None,
                lazy: false,
            });
        }
        let n1 = NodeId(Uuid::new_v4());
        let n2 = NodeId(Uuid::new_v4());
        infra.nodes.push(Node {
            id: n1,
            name: "a".into(),
            description: None,
            body: NodeBody::Shell(ShellOp::Run {
                argv: vec!["true".into()],
                cwd: None,
                env: Vec::new(),
            }),
            targets: Targets::Machine(a),
            deps: vec![],
            tags: vec![],
            policy: Default::default(),
        });
        infra.nodes.push(Node {
            id: n2,
            name: "b".into(),
            description: None,
            body: NodeBody::Shell(ShellOp::Run {
                argv: vec!["true".into()],
                cwd: None,
                env: Vec::new(),
            }),
            targets: Targets::Machine(b),
            deps: vec![n1],
            tags: vec![],
            policy: Default::default(),
        });
        let plan = infra.plan().unwrap();
        assert!(plan.slice_for_machine(&infra, b, SliceMode::Pull).is_err());
    }
}
