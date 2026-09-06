FROM rust:alpine3.20 as build_backend

RUN apk add musl-dev gcc

COPY ./backend ./backend
WORKDIR /backend

RUN cargo build --release

FROM alpine:3.20

RUN apk update \
    && apk add openssl ca-certificates libc6-compat

ENV NIMBUS_USER_ID=112
RUN addgroup -S nimbus-group && adduser -S nimbus-user -G nimbus-group -u $NIMBUS_USER_ID
USER nimbus-user

COPY --chown=nimbus-user --from=build_backend ./backend/target/release/backend  ./app/backend
COPY --chown=nimbus-user --from=build_backend ./backend/dist ./app/dist

WORKDIR /app

EXPOSE 8050
CMD ["./backend"]
