FROM debian:bookworm-slim
WORKDIR /app
COPY target/release/valine_bot /app/valine_bot
CMD ["./valine_bot"]
