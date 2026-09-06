from pydantic import BaseModel, Field


class GlossaryItem(BaseModel):
    name: str
    definition: str = Field("", description="The definition of the item.")


class Glossary(BaseModel):
    glossary: list[GlossaryItem]
