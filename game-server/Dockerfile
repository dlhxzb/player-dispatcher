FROM ubuntu:20.04    
# FROM nexus.jp.sbibits.com:8442/library/ubuntu:20.04
RUN mkdir -p /app
COPY ../target/release/game-server /app   
WORKDIR /app
CMD /app/game-server