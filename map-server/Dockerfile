FROM ubuntu:20.04    
#FROM nexus.jp.sbibits.com:8442/library/ubuntu:20.04
RUN mkdir -p /app
COPY ../target/release/map-server /app   
WORKDIR /app
CMD /app/map-server