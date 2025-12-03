#!/bin/bash

just docker-build mojave-sequencer ""
just docker-build mojave-node ""
just docker-build mojave-prover ""

kubectl delete -f k8s/ --ignore-not-found=true

kubectl apply -f k8s/namespace.yaml
kubectl apply -f k8s/secret.sequencer.yaml
kubectl apply -f k8s/rbac.sequencer.yaml
kubectl apply -f k8s/service.sequencer.yaml
kubectl apply -f k8s/stateful.sequencer.yaml
kubectl apply -f k8s/deploy.node.yaml
