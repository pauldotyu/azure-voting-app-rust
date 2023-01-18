## Setup Environment
az deployment sub create --template-file ./deploy/main.bicep --location eastus
$AcrName = az deployment sub show --name main --query 'properties.outputs.acr_name.value' -o tsv
$AksName = az deployment sub show --name main --query 'properties.outputs.aks_name.value' -o tsv
$ResourceGroup = az deployment sub show --name main --query 'properties.outputs.resource_group_name.value' -o tsv

az acr build --registry $AcrName --% --image cnny2023/azure-voting-app-rust:{{.Run.ID}} .
$BuildTag = az acr repository show-tags --name $AcrName --repository cnny2023/azure-voting-app-rust --orderby time_desc --query '[0]' -o tsv

az aks get-credentials --resource-group $ResourceGroup --name $AksName

## Pods
kubectl run azure-voting-db --image "postgres:15.0-alpine" --env "POSTGRES_PASSWORD=mypassword" --output yaml --dry-run=client > manifests/pod-db.yaml
kubectl apply -f ./manifests/pod-db.yaml

$DB_IP = kubectl get pod azure-voting-db -o jsonpath='{.status.podIP}'

kubectl run azure-voting-app --image "$AcrName.azurecr.io/cnny2023/azure-voting-app-rust:$BuildTag"  --env "DATABASE_URL=postgres://postgres:mypassword@$DB_IP" --output yaml  --dry-run=client > manifests/pod-app.yaml
kubectl apply -f ./manifests/pod-app.yaml

## Clean up
kubectl delete -f ./manifests/pod-app.yaml
kubectl delete -f ./manifests/pod-db.yaml

## Deployments
kubectl create deployment azure-voting-db --image "postgres:15.0-alpine" --port 5432 --output yaml --dry-run=client > manifests/deployment-db.yaml
# Edit file to add environment variable
# env:
# - name: POSTGRES_PASSWORD
#   value: "mypassword"
kubectl apply -f ./manifests/deployment-db.yaml

kubectl create deployment azure-voting-app --image "$AcrName.azurecr.io/cnny2023/azure-voting-app-rust:$BuildTag" --port 8080 --output yaml  --dry-run=client > manifests/deployment-app.yaml
kubectl get pod --selector app=azure-voting-db -o jsonpath='{.items[0].status.podIP}'

# Edit file to add environment variable and replace $DB_IP with the IP for the database pod:
# env:
# - name: DATABASE_URL
#   value: postgres://postgres:mypassword@$DB_IP
kubectl apply -f ./manifests/deployment-app.yaml

## Clean up
kubectl delete -f ./manifests/deployment-app.yaml
kubectl delete -f ./manifests/deployment-db.yaml

az group delete --name $ResourceGroup --no-wait --yes
