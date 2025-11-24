## Kubernetes guide (with Minikube) for Mojave

This document explains:

- Where Mojave currently uses Kubernetes (today this is focused on the Sequencer for high availability).
- How to spin up a local Minikube cluster, deploy the Sequencer with leader election, and test failover using the manifests in `k8s/`.

### 1. How Mojave uses Kubernetes

#### 1.1 Scope

- Kubernetes is currently used for **highly-available Sequencer deployment**.
- The provided manifests run **3 Sequencer pods** behind a `ClusterIP` service.
- At any moment, **only one pod is the leader** and runs the "leader tasks"; the other pods stay ready to take over if the leader fails.
- Other components (Node, Prover, etc.) can be run as regular processes or containers; this repository does not currently ship production-ready Kubernetes manifests for them.

#### 1.2 Packaged Kubernetes resources

All manifests live under `k8s/`:

- `k8s/deploy.sequencer.yaml`: `Deployment` with 3 replicas of `mojave-sequencer`, configured for Kubernetes leader election and persistent storage.
- `k8s/service.sequencer.yaml`: `ClusterIP` `Service` exposing:
  - HTTP JSON-RPC on port `18545` (Currently not using).
  - P2P networking on port `30305` (TCP and UDP).
- `k8s/rbac.sequencer.yaml`:
  - `ServiceAccount` (`sequencer-sa`).
  - `Role` and `RoleBinding` that grant access to `coordination.k8s.io/v1` `Lease` objects.
- `k8s/pvc.yaml`:
  - `PersistentVolume` (`sequencer-pv`) that uses a `hostPath` directory **inside the Minikube node** at `/data/mojave`.
  - `PersistentVolumeClaim` (`sequencer-pvc`) bound to that PV.
- `k8s/setup.sh`: Helper script that deletes everything under `k8s/` and then re-applies the core resources (PVC, RBAC, Service, Deployment).

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

The data directory is backed by the PVC:

- The PV on the Minikube node uses the host path `/data/mojave`.
- Inside the container, this PV is mounted at `/data/mojave`.
- The Sequencer is started with `--datadir /data/mojave/sequencer`, so the effective data directory is `/data/mojave/sequencer` on the Minikube node.

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

#### 2.3 Prepare persistent storage inside Minikube

The `k8s/pvc.yaml` manifest expects a hostPath directory at `/data/mojave` on the Minikube node. Create it and make it writable:

```bash
minikube ssh "sudo mkdir -p /data/mojave && sudo chmod 777 /data/mojave"
```

Notes:

- This directory lives **inside the Minikube VM/container**, not on your host filesystem.
- The PV (`sequencer-pv`) references `/data/mojave`, and the Sequencer pod mounts that PV at `/data/mojave` inside the container.

#### 2.4 Choose a Sequencer Docker image

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

3. Update the Deployment to use your image instead of the default:

   - Option 1: Edit `k8s/deploy.sequencer.yaml`:

     ```yaml
     # inside k8s/deploy.sequencer.yaml
     containers:
       - name: mojave-sequencer
         image: <your-docker-username>/mojave-sequencer:latest
         imagePullPolicy: IfNotPresent
     ```

   - Option 2: Patch the live Deployment after applying the manifests:

     ```bash
     kubectl set image deployment/mojave-sequencer-deployment \
       mojave-sequencer=<your-docker-username>/mojave-sequencer:latest
     ```

#### 2.6 Apply the core Kubernetes resources

You can apply the manifests one by one:

```bash
kubectl apply -f k8s/pvc.yaml
kubectl apply -f k8s/rbac.sequencer.yaml
kubectl apply -f k8s/service.sequencer.yaml
kubectl apply -f k8s/deploy.sequencer.yaml
```

Or use the helper script (note: this **first deletes** all manifests under `k8s/`):

```bash
bash k8s/setup.sh
```

#### 2.7 Verify the deployment

Check that all resources are created:

```bash
kubectl get pods
kubectl get svc mojave-sequencer-service
kubectl get pvc sequencer-pvc
kubectl get lease sequencer-leader -o yaml
```

You should see:

- 3 pods with `app=mojave-sequencer`.
- A single `Lease` named `sequencer-leader` with one of the pods listed as the current holder.

#### 2.8 Check that the current leader is producing blocks (via logs)

Before testing failover, confirm that the current leader pod is actively producing blocks.

1. **Identify the current leader pod** (via the `Lease` holder identity):

   ```bash
   LEADER_POD=$(kubectl get lease sequencer-leader -o jsonpath='{.spec.holderIdentity}')
   echo "Current leader pod: $LEADER_POD"
   ```

2. **Print the last 100 log lines from the leader pod**:

   ```bash
   kubectl logs "$LEADER_POD" -c mojave-sequencer | tail -n 100
   ```

Review the logs and confirm that the leader is producing blocks (or whatever periodic work you expect the Sequencer leader to perform).

#### 2.9 Leader failover test

To see Kubernetes-based HA in action:

1. **Delete the current leader pod** (identified in the previous step):

   ```bash
   kubectl delete pod "$LEADER_POD"
   ```

2. **Wait for a new leader to be elected and identify it**:

   ```bash
   # After some time (up to LEASE_TTL_SECONDS), check which pod is the new leader
   NEW_LEADER_POD=$(kubectl get lease sequencer-leader -o jsonpath='{.spec.holderIdentity}')
   echo "New leader pod: $NEW_LEADER_POD"

   # (Optional) list all sequencer pods
   kubectl get pods -l app=mojave-sequencer
   ```

3. **Print the last 100 log lines from the new leader pod**:

   ```bash
   kubectl logs "$NEW_LEADER_POD" -c mojave-sequencer | tail -n 100
   ```

Within approximately the lease TTL window (`LEASE_TTL_SECONDS`, default 15s), another pod should acquire the lease and start the leader tasks. By comparing the logs before and after the failover, you can verify that block production (or other leader activity) continues on the new leader.

#### 2.10 Cleanup

To remove all Mojave-related Kubernetes resources created from `k8s/`:

```bash
kubectl delete -f k8s/
```

---

### 3. Notes and recommendations (towards production)

The manifests in this repository are designed for **local development and testing**, not as a final production setup. For a production deployment, you should at least:

- Avoid passing private keys on the command line; prefer Kubernetes `Secret` objects and environment variables or mounted files.
- Configure proper resource requests and limits for each container.
- Add liveness and readiness probes for the Sequencer container to improve reliability and rollout behavior.
- Consider a `Headless Service` or separate Services if you need stable pod identities or direct pod-to-pod communication.
- Configure `imagePullSecrets` if you use private container registries.
- Use a production-grade storage class instead of `hostPath` volumes.
