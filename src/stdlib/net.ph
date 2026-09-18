# stdlib/net.ph — Minimal networking
#
# NOTE: These builtins return an error unless the "net" Cargo feature is
# enabled.  The wrappers below give callers a clean PASTA interface so
# swapping in a real implementation requires no user-code changes.
#
# Exports:
#   net.get(url)              -> string   HTTP GET body
#   net.post(url, body)       -> string   HTTP POST body
#   net.connect(host, port)   -> handle   open TCP connection
#   net.send(conn, data)                  send data on connection
#   net.recv(conn)            -> string   receive data
#   net.close(conn)                       close connection

set __header_net = "net loaded"

DEF net.get(url):             RET.NOW(): net.get(url)              END
DEF net.post(url, body):      RET.NOW(): net.post(url, body)       END
DEF net.connect(host, port):  RET.NOW(): net.connect(host, port)   END
DEF net.send(conn, data):     net.send(conn, data)                 END
DEF net.recv(conn):           RET.NOW(): net.recv(conn)            END
DEF net.close(conn):          net.close(conn)                      END
