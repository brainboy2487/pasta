# Makefile for Pasta dev tasks
.PHONY: build smoke install dev-setup golden-check ui-demo

build:
	cargo build --release

smoke:
	python3 tools/devkit/tasks.py smoke

golden-check:
	python3 tools/devkit/tasks.py golden-check

install:
	bash tools/devkit/install_pasta.sh

dev-setup:
	python3 -m venv .venv || true
	. .venv/bin/activate && pip install --upgrade pip
	@echo "Dev environment ready. Activate with: . .venv/bin/activate"

ui-demo:
	python3 tools/devkit/ui/ui_demo.py
