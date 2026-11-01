# Development candidate. Both images must be supplied by the caller.
# No unverified image tag/digest or implicit Dockerfile frontend is selected.
ARG BUILDER_IMAGE
ARG RUNTIME_IMAGE
FROM ${BUILDER_IMAGE} AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY LICENSE NOTICE.md ./
COPY LICENSES ./LICENSES
RUN set -eu; \
    case "$(rustc --version)" in \
        "rustc 1.96."*) ;; \
        *) echo "This candidate requires caller-supplied Rust 1.96.x" >&2; exit 1 ;; \
    esac; \
    cargo build --locked --release --package recastpress-server --bin recastpress

FROM ${RUNTIME_IMAGE} AS runtime
WORKDIR /app
COPY --from=builder /build/target/release/recastpress /usr/local/bin/recastpress
COPY --from=builder /build/LICENSE /build/NOTICE.md /usr/share/doc/recastpress/
COPY --from=builder /build/LICENSES /usr/share/doc/recastpress/LICENSES
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/recastpress"]
