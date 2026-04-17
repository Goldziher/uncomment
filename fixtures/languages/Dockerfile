# This comment should be removed
FROM ubuntu:22.04

# TODO: pin exact version
RUN apt-get update

ENV GREETING="Hello # not a comment"
LABEL description="Test image # metadata"

# hadolint ignore=DL3008
RUN apt-get install -y curl

COPY . /app
CMD ["./app"]
