# The only way out of the dev container: an HTTPS proxy letting through the domains in
# allowed-domains.txt and nothing else
FROM alpine:3.22

RUN apk add --no-cache squid

COPY squid.conf /etc/squid/squid.conf

USER squid
CMD ["squid", "-N", "-f", "/etc/squid/squid.conf"]
