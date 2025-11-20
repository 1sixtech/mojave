kubectl --kubeconfig=/Users/Giwook/.kube/mojave-sequencer-test.yml delete -f k8s/

kubectl --kubeconfig=/Users/Giwook/.kube/mojave-sequencer-test.yml apply -f k8s/ovh.pvc.yaml
kubectl --kubeconfig=/Users/Giwook/.kube/mojave-sequencer-test.yml apply -f k8s/rbac.sequencer.yaml
kubectl --kubeconfig=/Users/Giwook/.kube/mojave-sequencer-test.yml apply -f k8s/service.sequencer.yaml
kubectl --kubeconfig=/Users/Giwook/.kube/mojave-sequencer-test.yml apply -f k8s/deploy.sequencer.yaml