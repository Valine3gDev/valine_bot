FROM debian:bookworm-slim AS builder

ARG TARGETPLATFORM

WORKDIR /builder
COPY dist dist

RUN set -eu; \
    case "$TARGETPLATFORM" in \
        "linux/amd64") target="x86_64-unknown-linux-gnu" ;; \
        "linux/arm64") target="aarch64-unknown-linux-gnu" ;; \
        *) echo "Unsupported TARGETPLATFORM=$TARGETPLATFORM" >&2; exit 1 ;; \
    esac; \
    cp dist/valine_bot-*-"$target" ./valine_bot; \
    cp "dist/THIRD_PARTY_LICENSES-$target" ./THIRD_PARTY_LICENSES; \
    cp dist/LICENSE ./LICENSE; \
    chmod +x ./valine_bot

FROM gcr.io/distroless/cc-debian12:nonroot
WORKDIR /app
COPY --from=builder /builder/valine_bot /app/valine_bot
COPY --from=builder /builder/LICENSE /app/LICENSE
COPY --from=builder /builder/THIRD_PARTY_LICENSES /app/THIRD_PARTY_LICENSES
ENTRYPOINT ["./valine_bot"]
