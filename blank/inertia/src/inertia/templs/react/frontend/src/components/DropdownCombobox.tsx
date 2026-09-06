"use client"

import * as React from "react"
import { Check, ChevronsUpDown } from "lucide-react"

import { cn } from "@/lib/utils"
import { Button } from "@/components/ui/button"
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command"
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover"

interface DropdownComboboxProps {
    placeholder?: string
    label?: string
    options: { value: string; label: string }[]
    empty_text?: string
    value?: string
    clearable?: boolean
    onValueChange?: (value: string) => void
}


export default function DropdownCombobox({
    placeholder = "Select...",
    label = "Select...",
    options,
    empty_text = "No options found.",
    onValueChange,
}: DropdownComboboxProps) {
  const [open, setOpen] = React.useState(false)
  const [internalValue, setInternalValue] = React.useState("")

  React.useEffect(() => {
    onValueChange && onValueChange(internalValue)
  }, [internalValue])

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          role="combobox"
          aria-expanded={open}
          className="w-[200px] justify-between"
          onKeyDown={
            (e) => {
              if (e.key === "Backspace") {
                e.preventDefault()
                setInternalValue("")
              }
            }
          }
        >
          {internalValue
            ? <div>{options.find((option) => option.value === internalValue)?.label}</div>
            : <div className="text-gray-500">{placeholder}</div>}
          <ChevronsUpDown className="opacity-50" />
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-[200px] p-0">
        <Command>
          <CommandInput placeholder={label} className="h-9" />
          <CommandList>
            <CommandEmpty>{empty_text}</CommandEmpty>
            <CommandGroup>
              {options.map((option) => (
                <CommandItem
                  key={option.value}
                  value={option.value}
                  onSelect={(currentValue) => {
                    setInternalValue(currentValue === internalValue ? "" : currentValue)
                    setOpen(false)
                  }}
                >
                  {option.label}
                  <Check
                    className={cn(
                      "ml-auto",
                      internalValue === option.value ? "opacity-100" : "opacity-0"
                    )}
                  />
                </CommandItem>
              ))}
            </CommandGroup>
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  )
}
