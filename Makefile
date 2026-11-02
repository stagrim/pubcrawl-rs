EXECUTABLES = cargo deno
K := $(foreach exec,$(EXECUTABLES),\
    $(if $(shell which $(exec)),some string,$(error "No '$(exec)' in PATH. Please install $(exec) or edit PATH and try again")))


# TODO: Seems to work, but maybe use air to notify proxy after tailwind has generated css file
# to avoid race condition.
live:
	deno run --allow-run --allow-env --allow-read npm:concurrently --kill-others \
      "cargo watch -w src -x run" \
      "deno run dev:watch"

install:
	deno i --allow-scripts
	cargo install cargo-watch
	@echo ""
	@echo "✅ Done!"
	@echo "Remember to add ~/.cargo/bin to you path"
	@echo ""

compress:
	find "./static" -type f -regex ".*\(css\|js\)$\" | parallel -j$(shell nproc) 'zstd -19fk {}'
	find "./static" -type f -regex ".*\(css\|js\)$\" | parallel -j$(shell nproc) 'gzip -9fk {}'
