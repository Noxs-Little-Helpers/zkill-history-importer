docker build -t zkill-ws-history ../
docker container stop zkill-ws-history
docker container rm zkill-ws-history
docker run -itd --name zkill-ws-history --network nlh-network zkill-ws-history