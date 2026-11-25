"use client"

import { useCallback, useEffect, useReducer, useState } from "react"

import { toAppIpcError, type AppIpcError } from "@/lib/ipc/client"

export interface AsyncResource<T> {
  data: T
  loading: boolean
  error: AppIpcError | null
  setData: (value: T) => void
  refresh: () => void
}

interface ResourceState<T> {
  data: T
  loading: boolean
  error: AppIpcError | null
}

type ResourceAction<T> =
  | { type: "loading" }
  | { type: "success"; data: T }
  | { type: "failure"; error: AppIpcError }
  | { type: "set"; data: T }

function reducer<T>(state: ResourceState<T>, action: ResourceAction<T>): ResourceState<T> {
  switch (action.type) {
    case "loading":
      return { ...state, loading: true, error: null }
    case "success":
      return { data: action.data, loading: false, error: null }
    case "failure":
      return { ...state, loading: false, error: action.error }
    case "set":
      return { ...state, data: action.data }
  }
}

export function useAsyncResource<T>(
  load: () => Promise<T>,
  initialValue: T,
): AsyncResource<T> {
  const [state, dispatch] = useReducer(reducer<T>, {
    data: initialValue,
    loading: true,
    error: null,
  })
  const [refreshToken, setRefreshToken] = useState(0)

  const setData = useCallback((value: T) => {
    dispatch({ type: "set", data: value })
  }, [])

  const refresh = useCallback(() => {
    setRefreshToken((token) => token + 1)
  }, [])

  useEffect(() => {
    let active = true
    dispatch({ type: "loading" })

    load()
      .then((data) => {
        if (active) {
          dispatch({ type: "success", data })
        }
      })
      .catch((error: unknown) => {
        if (active) {
          dispatch({ type: "failure", error: toAppIpcError(error) })
        }
      })

    return () => {
      active = false
    }
  }, [load, refreshToken])

  return { ...state, setData, refresh }
}
