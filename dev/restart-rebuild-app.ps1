docker build -t zkill-ws-importer ../
docker container stop zkill-ws-importer
docker container rm zkill-ws-importer
docker run -itd --name zkill-ws-importer --network nlh-network zkill-ws-importer