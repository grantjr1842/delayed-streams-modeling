import { useCallback, useEffect, useRef, useState } from "react";
import { decodeMessage, encodeMessage } from "../../../protocol/encoder";
import type { WSMessage } from "../../../protocol/types";

export const useSocket = ({
  onMessage,
  uri,
  onDisconnect: onDisconnectProp,
}: {
  onMessage?: (message: WSMessage) => void;
  uri: string;
  onDisconnect?: () => void;
}) => {
  const lastMessageTime = useRef<null | number>(null);
  const [isConnected, setIsConnected] = useState(false);
  const [socket, setSocket] = useState<WebSocket | null>(null);

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
    // setIsConnected(true);
  }, []);

  const onDisconnect = useCallback(() => {
    console.log("disconnected");
    if (onDisconnectProp) {
      onDisconnectProp();
    }
    setIsConnected(false);
  }, [onDisconnectProp]);

  const onMessageEvent = useCallback(
    (eventData: MessageEvent) => {
      lastMessageTime.current = Date.now();
      const dataArray = new Uint8Array(eventData.data);
      const message = decodeMessage(dataArray);
      if (message.type === "handshake") {
        console.log("Handshake received, let's rocknroll.");
        setIsConnected(true);
      }
      if (!onMessage) {
        return;
      }
      onMessage(message);
    },
    [onMessage],
  );

  const start = useCallback(() => {
    const ws = new WebSocket(uri);
    ws.binaryType = "arraybuffer";
    ws.addEventListener("open", onConnect);
    ws.addEventListener("close", onDisconnect);
    ws.addEventListener("message", onMessageEvent);
    setSocket(ws);
    console.log("Socket created", ws);
    lastMessageTime.current = Date.now();
  }, [uri, onConnect, onDisconnect, onMessageEvent]);

  const stop = useCallback(() => {
    setIsConnected(false);
    if (onDisconnectProp) {
      onDisconnectProp();
    }
    socket?.close();
    setSocket(null);
  }, [socket, onDisconnectProp]);

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
        socket?.close();
        onDisconnect();
        clearInterval(intervalId);
      }
    }, 500);

    return () => {
      lastMessageTime.current = null;
      clearInterval(intervalId);
    };
  }, [isConnected, socket, onDisconnect]);

  return {
    isConnected,
    socket,
    sendMessage,
    start,
    stop,
  };
};
