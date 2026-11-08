"""Loopback configuration for synthetic servers, isolated from ordinary app ports."""
import socket


def server_configuration():
    # Ask the OS for an unused loopback port, then let the child bind it normally.
    # If another process wins the short release/bind race, readiness must fail;
    # clients still use only this child's authenticated discovery record.
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as probe:
        probe.bind(('127.0.0.1', 0))
        port = probe.getsockname()[1]
    return f'server:\n  port: {port}\n  port_fallback_count: 0\n'
