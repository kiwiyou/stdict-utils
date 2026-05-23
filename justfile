#!/usr/bin/env -S just --justfile

stdict:
	mkdir -p vendor/stdict
	# 2026-05
	curl --data-urlencode link_key=1554060 -o vendor/stdict-xml.zip https://stdict.korean.go.kr/common/download.do
	unzip vendor/stdict-xml.zip -d vendor/stdict
	rm vendor/stdict-xml.zip
