#!/bin/bash

kubectl delete -f k8s/ --ignore-not-found=true

kubectl apply -f k8s/pvc.yaml
kubectl apply -f k8s/rbac.sequencer.yaml
kubectl apply -f k8s/service.sequencer.yaml
kubectl apply -f k8s/deploy.sequencer.yaml
