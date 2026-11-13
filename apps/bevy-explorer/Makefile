.PHONY: web web-dev headless

# python3 is a Microsoft Store stub on a stock Windows: make web PYTHON=python
PYTHON ?= python3

web:
	$(PYTHON) build-web.py

web-dev:
	$(PYTHON) build-web.py --no-opt

headless:
	$(PYTHON) build-headless.py
