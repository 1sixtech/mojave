## Kubernetes guide (with Minikube) for Mojave

This document explains:

- Where Mojave currently uses Kubernetes (today this is focused on the Sequencer for high availability).
- How to spin up a local Minikube cluster, deploy the Sequencer with leader election, and test failover using the manifests in `k8s/`.

### 1. How Mojave uses Kubernetes

#### 1.1 Scope

- Kubernetes is currently used for **highly-available Sequencer deployment**.
- The provided manifests run **multiple Sequencer pods** (2 by default) behind Services.
- At any moment, **only one pod is the leader** and runs the "leader tasks"; the other pods stay ready to take over if the leader fails.
- Other components (Node, Prover, etc.) can be run as regular processes or containers; this repository does not currently ship production-ready Kubernetes manifests for them.

#### 1.2 Packaged Kubernetes resources

All manifests live under `k8s/`:

- `k8s/namespace.yaml`: Namespace definition (`1sixtech`) for the Sequencer resources.
- `k8s/deploy.sequencer.yaml`: `StatefulSet` (2 replicas by default) of `mojave-sequencer`, with leader election and a `volumeClaimTemplate` (20Gi) per pod mounted at `/data/mojave`.
- `k8s/deploy.node.yaml`: `Deployment` (1 replica by default) of `mojave-node` with an init container that prepares `/data/mojave/node` and writes a random JWT secret for `authrpc`.
- `k8s/service.sequencer.yaml`:
  - Headless `Service` (`mojave-sequencer-headless`) for the StatefulSet `serviceName`.
  - `ClusterIP` `Service` exposing:
  - HTTP JSON-RPC on port `18545` (Currently not using).
  - P2P networking on port `30305` (TCP and UDP).
- `k8s/rbac.sequencer.yaml`:
  - `ServiceAccount` (`sequencer-sa`).
  - `Role` and `RoleBinding` that grant access to `coordination.k8s.io/v1` `Lease` objects.
- `k8s/setup.sh`: Helper script that deletes everything under `k8s/` and then re-applies the core resources (Namespace, Secret, RBAC, Services, StatefulSet).

#### 1.3 Sequencer behavior on Kubernetes

The Sequencer binary detects a Kubernetes environment by checking the `KUBERNETES_SERVICE_HOST` environment variable. When running in Kubernetes:

- The pod receives the following environment variables from `deploy.sequencer.yaml`:
  - `POD_NAME`: the pod name (from the downward API).
  - `POD_NAMESPACE`: the namespace.
  - `LEASE_NAME`: the name of the `Lease` object used for leader election (default: `sequencer-leader`).
  - `LEASE_TTL_SECONDS`: lease time to live (default: `15` seconds).
- The code in `cmd/sequencer/src/k8s_leader.rs` uses these to participate in leader election via `kube-leader-election`:
  - If the pod **acquires** the lease, it becomes the leader and starts the leader tasks:
    - Batch producer.
    - Block producer.
    - Proof coordinator.
    - Committer.
  - If the pod **loses** the lease, it steps down and stops the leader tasks.
- On shutdown, the Sequencer writes a `node_config.json` under its data directory so that state can be reused on restart.

The data directory is backed by a per-pod `PersistentVolumeClaim` generated from the StatefulSet's `volumeClaimTemplate`:

- Each pod gets its own 20Gi claim (using the cluster's default `StorageClass` unless you override `storageClassName`).
- The claim is mounted at `/data/mojave` in the container.
- The Sequencer is started with `--datadir /data/mojave/sequencer`, so each pod writes to its own `/data/mojave/sequencer`.
- An init container writes `/data/mojave/statefulset-ordinal` with the pod's ordinal (0, 1, …) and `/data/mojave/role` with `primary` for ordinal 0 and `secondary` otherwise. Adjust this script in `k8s/deploy.sequencer.yaml` if you want different per-pod config files.

---

### 2. Local Minikube setup for Sequencer HA test (step-by-step)

The following steps show how to run a **local, highly-available Sequencer** on Minikube using only commands you can copy and paste.

#### 2.1 Prerequisites

Make sure you have:

- Docker (or another container runtime supported by Minikube).
- `kubectl`.
- Minikube (Docker driver is recommended on macOS).
- Rust toolchain (if you plan to build Mojave locally).
- `just` (optional but recommended for building Docker images; see the repository `justfile`).

#### 2.2 Start Minikube

Start a Minikube cluster:

```bash
minikube start --driver=docker

# Optional: enable metrics-server for better visibility
minikube addons enable metrics-server
```

You only need to do this once per Minikube profile.

#### 2.3 Choose a Sequencer Docker image

For this HA test you need a Docker image that contains the `mojave-sequencer` binary. There are two main options:

**Option A (recommended for a quick test): Use the prebuilt Mojave image**

- `k8s/deploy.sequencer.yaml` already references a published image:
  - `starkgiwook/mojave-sequencer:latest`
- If your Minikube cluster can pull from Docker Hub, you **do not need to build anything**. You can skip to **2.6 Apply the core Kubernetes resources**.

**Option B: Build and push your own image under your Docker namespace**

1. Build the Sequencer image locally using the `justfile` helper:

   ```bash
   just docker-build mojave-sequencer
   ```

   This creates a local image tagged as `1sixtech/mojave-sequencer`.

2. Retag it into your own Docker namespace and push it:

   ```bash
   # Replace <your-docker-username> with your Docker Hub (or registry) username
   docker tag 1sixtech/mojave-sequencer <your-docker-username>/mojave-sequencer:latest
   docker push <your-docker-username>/mojave-sequencer:latest
   ```

3. Update the StatefulSet to use your image instead of the default:

   - Option 1: Edit `k8s/deploy.sequencer.yaml`:

     ```yaml
     # inside k8s/deploy.sequencer.yaml
     containers:
       - name: mojave-sequencer
         image: <your-docker-username>/mojave-sequencer:latest
         imagePullPolicy: IfNotPresent
     ```

   - Option 2: Patch the live StatefulSet after applying the manifests:

     ```bash
     kubectl set image statefulset/mojave-sequencer -n 1sixtech \
       mojave-sequencer=<your-docker-username>/mojave-sequencer:latest
     ```

#### 2.4 Apply the core Kubernetes resources

You can apply the manifests one by one:

```bash
kubectl apply -f k8s/namespace.yaml
kubectl apply -f k8s/secret.sequencer.yaml
kubectl apply -f k8s/rbac.sequencer.yaml
kubectl apply -f k8s/service.sequencer.yaml
kubectl apply -f k8s/deploy.sequencer.yaml
kubectl apply -f k8s/deploy.node.yaml
```

Or use the helper script (note: this **first deletes** all manifests under `k8s/`):

```bash
bash k8s/setup.sh
```

If you hit `pod has unbound immediate PersistentVolumeClaims` errors, make sure your cluster has a default `StorageClass` or set `storageClassName` in `k8s/deploy.sequencer.yaml` to one that exists in your cluster (e.g., `local-path` on k3d/k3s, `standard` on many managed clusters).

#### 2.5 Verify the deployment

Check that all resources are created:

```bash
kubectl get sts mojave-sequencer-deployment -n 1sixtech
kubectl get pods -n 1sixtech
kubectl get svc mojave-sequencer -n 1sixtech
kubectl get pvc -n 1sixtech
kubectl get lease sequencer-leader -n 1sixtech -o yaml
```

You should see:

- 2 pods (or however many replicas you configured) with `app=mojave-sequencer`.
- A StatefulSet `mojave-sequencer-deployment` reporting ready replicas.
- Per-pod PVCs named like `sequencer-datadir-mojave-sequencer-deployment-0`.
- A single `Lease` named `sequencer-leader` with one of the pods listed as the current holder.

#### 2.6 Check that the current leader is producing blocks (via logs)

Before testing failover, confirm that the current leader pod is actively producing blocks.

1. **Identify the current leader pod** (via the `Lease` holder identity):

   ```bash
   LEADER_POD=$(kubectl get lease sequencer-leader -n 1sixtech -o jsonpath='{.spec.holderIdentity}')
   echo "Current leader pod: $LEADER_POD"
   ```

2. **Print the last 100 log lines from the leader pod**:

   ```bash
   kubectl logs "$LEADER_POD" -c mojave-sequencer -n 1sixtech | tail -n 100
   ```

Review the logs and confirm that the leader is producing blocks (or whatever periodic work you expect the Sequencer leader to perform).

#### 2.7 Leader failover test

To see Kubernetes-based HA in action:

1. **Delete the current leader pod** (identified in the previous step):

   ```bash
   kubectl delete pod "$LEADER_POD" -n 1sixtech
   ```

2. **Wait for a new leader to be elected and identify it**:

   ```bash
   # After some time (up to LEASE_TTL_SECONDS), check which pod is the new leader
   NEW_LEADER_POD=$(kubectl get lease sequencer-leader -n 1sixtech -o jsonpath='{.spec.holderIdentity}')
   echo "New leader pod: $NEW_LEADER_POD"

   # (Optional) list all sequencer pods
   kubectl get pods -l app=mojave-sequencer -n 1sixtech
   ```

3. **Print the last 100 log lines from the new leader pod**:

   ```bash
   kubectl logs "$NEW_LEADER_POD" -c mojave-sequencer -n 1sixtech | tail -n 100
   ```

Within approximately the lease TTL window (`LEASE_TTL_SECONDS`, default 15s), another pod should acquire the lease and start the leader tasks. By comparing the logs before and after the failover, you can verify that block production (or other leader activity) continues on the new leader.

#### 2.8 Cleanup

To remove all Mojave-related Kubernetes resources created from `k8s/`:

```bash
kubectl delete -f k8s/
# Delete the per-pod PVCs created by the StatefulSet
kubectl delete pvc -l app=mojave-sequencer -n 1sixtech
```

---

### 3. Notes and recommendations (towards production)

The manifests in this repository are designed for **local development and testing**, not as a final production setup. For a production deployment, you should at least:

- Avoid passing private keys on the command line; prefer Kubernetes `Secret` objects and environment variables or mounted files.
- Configure proper resource requests and limits for each container.
- Add liveness and readiness probes for the Sequencer container to improve reliability and rollout behavior.
- Consider a `Headless Service` or separate Services if you need stable pod identities or direct pod-to-pod communication.
- Configure `imagePullSecrets` if you use private container registries.
- Use a production-grade storage class appropriate for your cluster/storage backend.
