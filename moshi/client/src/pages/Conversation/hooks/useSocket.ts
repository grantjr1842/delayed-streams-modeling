import { useCallback, useEffect, useRef, useState } from "react";
import { decodeMessage, encodeMessage } from "../../../protocol/encoder";
import type { WSMessage } from "../../../protocol/types";
import type { ConnectionError } from "../../../types/errors";
import {
    parseCloseEvent,
    parseErrorEvent,
    calculateRetryDelay,
} from "../../../utils/errors/parse-ws-error";

export const useSocket = ({
  onMessage,
  uri,
  onDisconnect: onDisconnectProp,
  onError: onErrorProp,
  maxRetries = 3,
}: {
  onMessage?: (message: WSMessage) => void;
  uri: string;
  onDisconnect?: () => void;
  onError?: (error: ConnectionError) => void;
  maxRetries?: number;
}) => {
  const lastMessageTime = useRef<null | number>(null);
  const [isConnected, setIsConnected] = useState(false);
  const [isConnecting, setIsConnecting] = useState(false);
  const [socket, setSocket] = useState<WebSocket | null>(null);
  const [error, setError] = useState<ConnectionError | null>(null);
  const [retryCount, setRetryCount] = useState(0);
  const retryTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const clearError = useCallback(() => {
    setError(null);
  }, []);

  const sendMessage = useCallback(
    (message: WSMessage) => {
      if (!socket || !isConnected) {
        console.log("socket not connected");
        return;
      }
      socket.send(encodeMessage(message));
    },
    [isConnected, socket],
  );

  const onConnect = useCallback(() => {
    console.log("connected, now waiting for handshake.");
    setIsConnecting(false);
    setRetryCount(0);
    clearError();
  }, [clearError]);

  const handleError = useCallback(
    (connectionError: ConnectionError) => {
      console.error("WebSocket error:", connectionError);
      setError(connectionError);
      setIsConnecting(false);
      if (onErrorProp) {
        onErrorProp(connectionError);
      }
    },
    [onErrorProp],
  );

  const onCloseEvent = useCallback(
    (event: CloseEvent) => {
      console.log("WebSocket closed:", event.code, event.reason);
      setIsConnected(false);
      setIsConnecting(false);

      // Parse the close event into a structured error
      const connectionError = parseCloseEvent(event);

      // Only set error if it's not a normal closure
      if (event.code !== 1000) {
        handleError(connectionError);
      }

      if (onDisconnectProp) {
        onDisconnectProp();
      }
    },
    [onDisconnectProp, handleError],
  );

  const onErrorEvent = useCallback(
    (event: Event) => {
      console.error("WebSocket error event:", event);
      const connectionError = parseErrorEvent(event);
      handleError(connectionError);
    },
    [handleError],
  );

  const onMessageEvent = useCallback(
    (eventData: MessageEvent) => {
      lastMessageTime.current = Date.now();
      const dataArray = new Uint8Array(eventData.data);
      const message = decodeMessage(dataArray);
      if (message.type === "handshake") {
        console.log("Handshake received, let's rocknroll.");
        setIsConnected(true);
        setIsConnecting(false);
      }
      if (!onMessage) {
        return;
      }
      onMessage(message);
    },
    [onMessage],
  );

  const start = useCallback(() => {
    // Clear any pending retry
    if (retryTimeoutRef.current) {
      clearTimeout(retryTimeoutRef.current);
      retryTimeoutRef.current = null;
    }

    setIsConnecting(true);
    clearError();

    const ws = new WebSocket(uri);
    ws.binaryType = "arraybuffer";
    ws.addEventListener("open", onConnect);
    ws.addEventListener("close", onCloseEvent);
    ws.addEventListener("error", onErrorEvent);
    ws.addEventListener("message", onMessageEvent);
    setSocket(ws);
    console.log("Socket created", ws);
    lastMessageTime.current = Date.now();
  }, [uri, onConnect, onCloseEvent, onErrorEvent, onMessageEvent, clearError]);

  const stop = useCallback(() => {
    // Clear any pending retry
    if (retryTimeoutRef.current) {
      clearTimeout(retryTimeoutRef.current);
      retryTimeoutRef.current = null;
    }

    setIsConnected(false);
    setIsConnecting(false);
    if (onDisconnectProp) {
      onDisconnectProp();
    }
    socket?.close(1000, "Client requested disconnect");
    setSocket(null);
  }, [socket, onDisconnectProp]);

  const retryConnection = useCallback(() => {
    if (retryCount >= maxRetries) {
      console.log("Max retries reached, not retrying");
      return;
    }

    const delay = calculateRetryDelay(retryCount);
    console.log(`Retrying connection in ${delay}ms (attempt ${retryCount + 1}/${maxRetries})`);

    setRetryCount((prev) => prev + 1);
    clearError();

    retryTimeoutRef.current = setTimeout(() => {
      start();
    }, delay);
  }, [retryCount, maxRetries, start, clearError]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (retryTimeoutRef.current) {
        clearTimeout(retryTimeoutRef.current);
      }
    };
  }, []);

  // Inactivity timeout
  useEffect(() => {
    if (!isConnected) {
      return;
    }
    const intervalId = setInterval(() => {
      if (
        lastMessageTime.current &&
        Date.now() - lastMessageTime.current > 10000
      ) {
        console.log("closing socket due to inactivity", socket);
        socket?.close(4006, "Client inactivity timeout");
        clearInterval(intervalId);
      }
    }, 500);

    return () => {
      lastMessageTime.current = null;
      clearInterval(intervalId);
    };
  }, [isConnected, socket]);

  return {
    isConnected,
    isConnecting,
    socket,
    sendMessage,
    start,
    stop,
    error,
    clearError,
    retryConnection,
    retryCount,
  };
};
