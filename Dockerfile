FROM debian:bookworm-slim AS builder

ARG TARGETPLATFORM

WORKDIR /builder
COPY binaries binaries

RUN case "$TARGETPLATFORM" in \
        "linux/amd64") mv binaries/x86_64-unknown-linux-gnu  ./valine_bot  ;; \
        "linux/arm64") mv binaries/aarch64-unknown-linux-gnu ./valine_bot ;; \
        *) echo "Unsupported TARGETPLATFORM=$TARGETPLATFORM" >&2; exit 1 ;; \
    esac && \
    chmod +x ./valine_bot

FROM gcr.io/distroless/cc-debian12:nonroot
WORKDIR /app
COPY --from=builder /builder/valine_bot /app/valine_bot
CMD ["./valine_bot"]
